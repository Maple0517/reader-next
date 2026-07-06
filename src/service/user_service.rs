use crate::app::config::AppConfig;
use crate::error::error::AppError;
use crate::model::user::User;
use crate::util::crypto::{gen_encrypted_password, random_string};
use crate::util::time::now_ts;
use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

const TOKEN_TTL_MS: i64 = 7 * 86_400 * 1000;

#[derive(Clone)]
pub struct UserService {
    cfg: AppConfig,
    users_path: PathBuf,
    data_root: PathBuf,
    pool: SqlitePool,
}

impl UserService {
    pub fn new(cfg: AppConfig, pool: SqlitePool) -> Self {
        let data_root = PathBuf::from(&cfg.storage_dir).join("data");
        let users_path = data_root.join("users.json");
        Self {
            cfg,
            users_path,
            data_root,
            pool,
        }
    }

    pub async fn migrate_legacy_users_from_json(&self) -> Result<(), AppError> {
        if !self.users_path.exists() {
            self.ensure_admin_user().await?;
            return Ok(());
        }
        if self.user_count().await? > 0 {
            self.ensure_admin_user().await?;
            return Ok(());
        }

        let data = fs::read_to_string(&self.users_path)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let mut users: HashMap<String, User> =
            serde_json::from_str(&data).map_err(|e| AppError::BadRequest(e.to_string()))?;
        normalize_legacy_user_keys(&mut users);
        migrate_legacy_users(&mut users);
        self.save_users(&users).await?;
        Ok(())
    }

    pub fn secure_enabled(&self) -> bool {
        self.cfg.secure
    }

    pub fn secure_key_required(&self) -> bool {
        !self.cfg.secure_key.is_empty()
    }

    pub fn secure_key_matches(&self, key: &str) -> bool {
        !self.cfg.secure_key.is_empty() && self.cfg.secure_key == key
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        is_login: bool,
        code: Option<&str>,
    ) -> Result<Value, AppError> {
        self.ensure_admin_user().await?;
        if let Some(mut user) = self.find_user(username).await? {
            if !is_login {
                return Err(AppError::BadRequest("用户名已被占用".to_string()));
            }
            let encrypted = gen_encrypted_password(password, &user.salt);
            if encrypted != user.password {
                return Err(AppError::BadRequest("密码错误".to_string()));
            }
            let login_data = self.save_new_session(&mut user).await?;
            return Ok(login_data);
        }

        if is_login {
            return Err(AppError::BadRequest("用户不存在".to_string()));
        }
        self.validate_new_user(username, password, code)?;
        let user_count = self.user_count().await?;
        if user_count as u32 >= self.cfg.user_limit {
            return Err(AppError::BadRequest("超过用户数上限".to_string()));
        }

        let salt = random_string(8);
        let encrypted = gen_encrypted_password(password, &salt);
        let now = now_ms();
        let mut user = User {
            username: username.to_string(),
            password: encrypted,
            salt,
            token: "".to_string(),
            last_login_at: now,
            created_at: now,
            enable_webdav: false,
            token_map: None,
            enable_local_store: false,
            enable_ai_model: false,
            is_admin: user_count == 0,
        };
        self.upsert_user_row(&user).await?;
        let login_data = self.save_new_session(&mut user).await?;
        Ok(login_data)
    }

    pub async fn logout(&self, access_token: &str) -> Result<(), AppError> {
        let (username, token) = parse_access_token(access_token)?;
        let user = self
            .find_user(&username)
            .await?
            .ok_or_else(|| AppError::BadRequest("系统错误".to_string()))?;
        sqlx::query("DELETE FROM user_sessions WHERE username=?1 AND token=?2")
            .bind(&username)
            .bind(&token)
            .execute(&self.pool)
            .await?;
        if user.token == token {
            sqlx::query("UPDATE users SET token='' WHERE username=?1")
                .bind(&username)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn get_user_info(
        &self,
        access_token: Option<&str>,
        secure_key: Option<&str>,
    ) -> Result<(Option<Value>, bool, bool, bool), AppError> {
        let admin_authorized = self.is_admin(access_token, secure_key).await?;
        if let Some(token) = access_token {
            match self.check_auth(token).await {
                Ok(Some(user)) => {
                    return Ok((
                        Some(self.format_user(&user)),
                        self.cfg.secure,
                        self.secure_key_required(),
                        admin_authorized,
                    ))
                }
                Ok(None) => {}
                Err(err) if self.cfg.secure => return Err(err),
                Err(_) => {}
            }
        }
        Ok((
            None,
            self.cfg.secure,
            self.secure_key_required(),
            admin_authorized,
        ))
    }

    pub async fn save_user_config(&self, user_ns: &str, config: Value) -> Result<(), AppError> {
        let dir = self.data_root.join(user_ns);
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let path = dir.join("userConfig.json");
        let mut cfg = config;
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert("@updateTime".to_string(), Value::from(now_ts() * 1000));
        }
        let data = serde_json::to_string(&cfg).map_err(|e| AppError::BadRequest(e.to_string()))?;
        fs::write(path, data)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        Ok(())
    }

    pub async fn get_user_config(&self, user_ns: &str) -> Result<Value, AppError> {
        let path = self.data_root.join(user_ns).join("userConfig.json");
        if !path.exists() {
            return Err(AppError::BadRequest("没有备份文件".to_string()));
        }
        let data = fs::read_to_string(path)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let v: Value =
            serde_json::from_str(&data).map_err(|e| AppError::BadRequest(e.to_string()))?;
        Ok(v)
    }

    pub async fn get_user_list(&self) -> Result<Vec<Value>, AppError> {
        let users = self.load_users().await?;
        Ok(users.values().map(|u| self.format_user(u)).collect())
    }

    pub async fn add_user(&self, username: &str, password: &str) -> Result<Vec<Value>, AppError> {
        self.validate_new_user(username, password, None)?;
        if self.find_user(username).await?.is_some() {
            return Err(AppError::BadRequest("用户已存在".to_string()));
        }
        if self.user_count().await? as u32 >= self.cfg.user_limit {
            return Err(AppError::BadRequest("超过用户数上限".to_string()));
        }
        let salt = random_string(8);
        let encrypted = gen_encrypted_password(password, &salt);
        let now = now_ms();
        let user = User {
            username: username.to_string(),
            password: encrypted,
            salt,
            token: "".to_string(),
            last_login_at: now,
            created_at: now,
            enable_webdav: false,
            token_map: None,
            enable_local_store: false,
            enable_ai_model: false,
            is_admin: false,
        };
        self.upsert_user_row(&user).await?;
        self.get_user_list().await
    }

    pub async fn reset_password(&self, username: &str, password: &str) -> Result<(), AppError> {
        if password.len() < 8 {
            return Err(AppError::BadRequest("密码不能低于8位".to_string()));
        }
        if username == "default" {
            return Err(AppError::BadRequest("用户不存在".to_string()));
        }
        let mut user = self
            .find_user(username)
            .await?
            .ok_or_else(|| AppError::BadRequest("用户不存在".to_string()))?;
        let salt = random_string(8);
        let encrypted = gen_encrypted_password(password, &salt);
        user.salt = salt;
        user.password = encrypted;
        self.upsert_user_row(&user).await?;
        Ok(())
    }

    pub async fn change_password(
        &self,
        access_token: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        if new_password.len() < 8 {
            return Err(AppError::BadRequest("密码不能低于8位".to_string()));
        }
        let (username, token) = parse_access_token(access_token)?;
        let mut user = self
            .find_user(&username)
            .await?
            .ok_or_else(|| AppError::BadRequest("用户不存在".to_string()))?;
        let encrypted_old = gen_encrypted_password(old_password, &user.salt);
        if encrypted_old != user.password {
            return Err(AppError::BadRequest("当前密码错误".to_string()));
        }

        let salt = random_string(8);
        let encrypted = gen_encrypted_password(new_password, &salt);
        let now = now_ms();
        user.salt = salt;
        user.password = encrypted;
        user.token = token.clone();
        user.last_login_at = now;
        self.upsert_user_row(&user).await?;
        sqlx::query("DELETE FROM user_sessions WHERE username=?1 AND token<>?2")
            .bind(&username)
            .bind(&token)
            .execute(&self.pool)
            .await?;
        self.upsert_session(&username, &token, session_expire_at(now))
            .await?;
        Ok(())
    }

    pub async fn delete_users(&self, usernames: &[String]) -> Result<Vec<Value>, AppError> {
        for u in usernames {
            sqlx::query("DELETE FROM user_sessions WHERE username=?1")
                .bind(u)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM users WHERE username=?1")
                .bind(u)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM book_sources WHERE user_ns=?1")
                .bind(u)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM json_documents WHERE namespace=?1")
                .bind(u)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM ai_book_memories WHERE user_ns=?1")
                .bind(u)
                .execute(&self.pool)
                .await?;
            let user_dir = self.data_root.join(u);
            if user_dir.exists() {
                let _ = fs::remove_dir_all(user_dir).await;
            }
        }
        self.get_user_list().await
    }

    pub async fn update_user(
        &self,
        username: &str,
        enable_webdav: Option<bool>,
        enable_local_store: Option<bool>,
        enable_ai_model: Option<bool>,
    ) -> Result<Vec<Value>, AppError> {
        let mut user = self
            .find_user(username)
            .await?
            .ok_or_else(|| AppError::BadRequest("用户不存在".to_string()))?;
        if let Some(v) = enable_webdav {
            user.enable_webdav = v;
        }
        if let Some(v) = enable_local_store {
            user.enable_local_store = v;
        }
        if let Some(v) = enable_ai_model {
            user.enable_ai_model = v;
        }
        self.upsert_user_row(&user).await?;
        self.get_user_list().await
    }

    pub async fn check_auth(&self, access_token: &str) -> Result<Option<User>, AppError> {
        self.ensure_admin_user().await?;
        let (username, token) = parse_access_token(access_token)?;
        let mut user = match self.find_user(&username).await? {
            Some(u) => u,
            None => return Ok(None),
        };
        let now = now_ms();
        self.delete_expired_sessions(&username, now).await?;
        let row = sqlx::query("SELECT expire_at FROM user_sessions WHERE username=?1 AND token=?2")
            .bind(&username)
            .bind(&token)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let expire_at: i64 = row.get("expire_at");
        if expire_at <= now {
            sqlx::query("DELETE FROM user_sessions WHERE username=?1 AND token=?2")
                .bind(&username)
                .bind(&token)
                .execute(&self.pool)
                .await?;
            return Ok(None);
        }

        user.token = token.clone();
        user.last_login_at = now;
        self.update_user_session_fields(&username, &token, now)
            .await?;
        self.upsert_session(&username, &token, session_expire_at(now))
            .await?;
        user.token_map = Some(self.load_session_map(&username).await?);
        Ok(Some(user))
    }

    /// Check if user is admin (either by is_admin flag or by secure key)
    pub async fn is_admin(
        &self,
        access_token: Option<&str>,
        secure_key: Option<&str>,
    ) -> Result<bool, AppError> {
        if let Some(key) = secure_key {
            if self.secure_key_matches(key) {
                return Ok(true);
            }
        }
        if let Some(token) = access_token {
            if let Ok(Some(user)) = self.check_auth(token).await {
                return Ok(user.is_admin);
            }
        }
        Ok(false)
    }

    pub async fn can_use_ai_model(
        &self,
        access_token: Option<&str>,
        secure_key: Option<&str>,
    ) -> Result<bool, AppError> {
        if !self.cfg.secure {
            return Ok(true);
        }
        if let Some(key) = secure_key {
            if self.secure_key_matches(key) {
                return Ok(true);
            }
        }
        if let Some(token) = access_token {
            if let Ok(Some(user)) = self.check_auth(token).await {
                return Ok(user.is_admin || user.enable_ai_model);
            }
        }
        Ok(false)
    }

    pub async fn resolve_user_ns_with_override(
        &self,
        access_token: Option<&str>,
        secure_key: Option<&str>,
        user_ns: Option<&str>,
    ) -> Result<String, AppError> {
        if self.cfg.secure {
            if let Some(key) = secure_key {
                if self.secure_key_matches(key) {
                    if let Some(ns) = user_ns {
                        let ns = ns.trim();
                        if !ns.is_empty() {
                            return Ok(ns.to_string());
                        }
                    }
                }
            }
        }
        if let Some(token) = access_token {
            if let Ok(Some(user)) = self.check_auth(token).await {
                return Ok(user.username);
            }
        }
        if self.cfg.secure {
            if let Some(key) = secure_key {
                if self.secure_key_matches(key) {
                    return Ok("default".to_string());
                }
            }
        }
        if !self.cfg.secure {
            return Ok("default".to_string());
        }
        Err(AppError::BadRequest("NEED_LOGIN".to_string()))
    }

    pub async fn verify_basic_webdav(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<User>, AppError> {
        if !self.cfg.secure {
            return Ok(None);
        }
        let user = match self.find_user(username).await? {
            Some(u) => u,
            None => return Ok(None),
        };
        if !user.enable_webdav {
            return Ok(None);
        }
        let encrypted = gen_encrypted_password(password, &user.salt);
        if encrypted != user.password {
            return Ok(None);
        }
        Ok(Some(user))
    }

    pub async fn require_webdav_user(
        &self,
        access_token: Option<&str>,
    ) -> Result<String, AppError> {
        if !self.cfg.secure {
            return Err(AppError::BadRequest("仅安全模式支持WebDAV功能".to_string()));
        }
        let token = access_token.ok_or_else(|| AppError::BadRequest("NEED_LOGIN".to_string()))?;
        let user = self
            .check_auth(token)
            .await?
            .ok_or_else(|| AppError::BadRequest("NEED_LOGIN".to_string()))?;
        if !user.enable_webdav {
            return Err(AppError::BadRequest("未开启webdav功能".to_string()));
        }
        Ok(user.username)
    }

    async fn save_new_session(&self, user: &mut User) -> Result<Value, AppError> {
        let now = now_ms();
        user.last_login_at = now;
        user.token = self.generate_session_token(&user.username);
        self.update_user_session_fields(&user.username, &user.token, now)
            .await?;
        self.delete_expired_sessions(&user.username, now).await?;
        self.upsert_session(&user.username, &user.token, session_expire_at(now))
            .await?;
        user.token_map = Some(self.load_session_map(&user.username).await?);
        Ok(self.format_user(user))
    }

    fn format_user(&self, user: &User) -> Value {
        serde_json::json!({
            "username": user.username,
            "lastLoginAt": user.last_login_at,
            "accessToken": format!("{}:{}", user.username, user.token),
            "enableWebdav": user.enable_webdav,
            "enableLocalStore": user.enable_local_store,
            "enableAiModel": user.enable_ai_model,
            "createdAt": user.created_at,
            "isAdmin": user.is_admin,
        })
    }

    fn collect_active_sessions(&self, user: &User, now: i64) -> HashMap<String, i64> {
        let mut map = user.token_map.clone().unwrap_or_default();
        map.retain(|_, exp| *exp > now);

        if !user.token.is_empty() {
            let legacy_expire_at = user.last_login_at.saturating_add(TOKEN_TTL_MS);
            if legacy_expire_at > now {
                match map.get_mut(&user.token) {
                    Some(exp) if *exp < legacy_expire_at => *exp = legacy_expire_at,
                    Some(_) => {}
                    None => {
                        map.insert(user.token.clone(), legacy_expire_at);
                    }
                }
            }
        }

        map
    }

    fn generate_session_token(&self, username: &str) -> String {
        gen_encrypted_password(username, &format!("{}{}", now_ms(), random_string(8)))
    }

    async fn load_users(&self) -> Result<HashMap<String, User>, AppError> {
        self.ensure_admin_user().await?;
        self.load_users_raw().await
    }

    async fn load_users_raw(&self) -> Result<HashMap<String, User>, AppError> {
        let rows = sqlx::query(
            "SELECT username, password, salt, token, last_login_at, created_at, enable_webdav, \
             enable_local_store, enable_ai_model, is_admin FROM users ORDER BY created_at ASC, username ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut users = HashMap::new();
        for row in rows {
            let mut user = self.user_from_row(&row);
            user.token_map = Some(self.load_session_map(&user.username).await?);
            users.insert(user.username.clone(), user);
        }
        Ok(users)
    }

    async fn save_users(&self, users: &HashMap<String, User>) -> Result<(), AppError> {
        sqlx::query("DELETE FROM user_sessions")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM users").execute(&self.pool).await?;
        let now = now_ms();
        for (username, user) in users {
            let mut user = user.clone();
            if user.username.is_empty() {
                user.username = username.clone();
            }
            self.upsert_user_row(&user).await?;
            let sessions = self.collect_active_sessions(&user, now);
            for (token, expire_at) in sessions {
                self.upsert_session(&user.username, &token, expire_at)
                    .await?;
            }
        }
        self.ensure_admin_user().await?;
        Ok(())
    }

    async fn find_user(&self, username: &str) -> Result<Option<User>, AppError> {
        let row = sqlx::query(
            "SELECT username, password, salt, token, last_login_at, created_at, enable_webdav, \
             enable_local_store, enable_ai_model, is_admin FROM users WHERE username=?1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut user = self.user_from_row(&row);
        user.token_map = Some(self.load_session_map(&user.username).await?);
        Ok(Some(user))
    }

    fn user_from_row(&self, row: &SqliteRow) -> User {
        User {
            username: row.get("username"),
            password: row.get("password"),
            salt: row.get("salt"),
            token: row.get("token"),
            last_login_at: row.get("last_login_at"),
            created_at: row.get("created_at"),
            enable_webdav: row.get::<i64, _>("enable_webdav") != 0,
            token_map: None,
            enable_local_store: row.get::<i64, _>("enable_local_store") != 0,
            enable_ai_model: row.get::<i64, _>("enable_ai_model") != 0,
            is_admin: row.get::<i64, _>("is_admin") != 0,
        }
    }

    async fn upsert_user_row(&self, user: &User) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO users (username, password, salt, token, last_login_at, created_at, enable_webdav, enable_local_store, enable_ai_model, is_admin) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(username) DO UPDATE SET password=excluded.password, salt=excluded.salt, token=excluded.token, \
             last_login_at=excluded.last_login_at, created_at=excluded.created_at, enable_webdav=excluded.enable_webdav, \
             enable_local_store=excluded.enable_local_store, enable_ai_model=excluded.enable_ai_model, is_admin=excluded.is_admin",
        )
        .bind(&user.username)
        .bind(&user.password)
        .bind(&user.salt)
        .bind(&user.token)
        .bind(user.last_login_at)
        .bind(user.created_at)
        .bind(bool_to_i64(user.enable_webdav))
        .bind(bool_to_i64(user.enable_local_store))
        .bind(bool_to_i64(user.enable_ai_model))
        .bind(bool_to_i64(user.is_admin))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_user_session_fields(
        &self,
        username: &str,
        token: &str,
        last_login_at: i64,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET token=?1, last_login_at=?2 WHERE username=?3")
            .bind(token)
            .bind(last_login_at)
            .bind(username)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn user_count(&self) -> Result<i64, AppError> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("count"))
    }

    async fn ensure_admin_user(&self) -> Result<(), AppError> {
        let row = sqlx::query("SELECT username FROM users WHERE is_admin=1 LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        if row.is_some() {
            return Ok(());
        }
        let row = sqlx::query(
            "SELECT username FROM users ORDER BY CASE WHEN created_at <= 0 THEN 9223372036854775807 ELSE created_at END ASC, username ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = row {
            let username: String = row.get("username");
            sqlx::query("UPDATE users SET is_admin=1 WHERE username=?1")
                .bind(username)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn load_session_map(&self, username: &str) -> Result<HashMap<String, i64>, AppError> {
        let rows = sqlx::query(
            "SELECT token, expire_at FROM user_sessions WHERE username=?1 AND expire_at>?2",
        )
        .bind(username)
        .bind(now_ms())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("token"),
                    row.get::<i64, _>("expire_at"),
                )
            })
            .collect())
    }

    async fn upsert_session(
        &self,
        username: &str,
        token: &str,
        expire_at: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_sessions (username, token, expire_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(username, token) DO UPDATE SET expire_at=excluded.expire_at",
        )
        .bind(username)
        .bind(token)
        .bind(expire_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_expired_sessions(&self, username: &str, now: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM user_sessions WHERE username=?1 AND expire_at<=?2")
            .bind(username)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn validate_new_user(
        &self,
        username: &str,
        password: &str,
        code: Option<&str>,
    ) -> Result<(), AppError> {
        if username.is_empty() {
            return Err(AppError::BadRequest("请输入用户名".to_string()));
        }
        if password.is_empty() {
            return Err(AppError::BadRequest("请输入密码".to_string()));
        }
        if username.len() < 5 {
            return Err(AppError::BadRequest("用户名不能低于5位".to_string()));
        }
        if password.len() < 8 {
            return Err(AppError::BadRequest("密码不能低于8位".to_string()));
        }
        if username == "default" {
            return Err(AppError::BadRequest("用户名不能为非法字符".to_string()));
        }
        let re = regex::Regex::new("^[a-z0-9]+$").unwrap();
        if !re.is_match(username) {
            return Err(AppError::BadRequest(
                "用户名只能由字母和数字组成".to_string(),
            ));
        }
        if !self.cfg.invite_code.is_empty() {
            let c = code.unwrap_or("");
            if c.is_empty() {
                return Err(AppError::BadRequest("请输入邀请码".to_string()));
            }
            if c != self.cfg.invite_code {
                return Err(AppError::BadRequest("邀请码错误".to_string()));
            }
        }
        Ok(())
    }
}

fn parse_access_token(access_token: &str) -> Result<(String, String), AppError> {
    let parts: Vec<&str> = access_token.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest("NEED_LOGIN".to_string()));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn now_ms() -> i64 {
    now_ts() * 1000
}

fn session_expire_at(now: i64) -> i64 {
    now.saturating_add(TOKEN_TTL_MS)
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn normalize_legacy_user_keys(users: &mut HashMap<String, User>) {
    for (username, user) in users.iter_mut() {
        if user.username.is_empty() {
            user.username = username.clone();
        }
    }
}

fn migrate_legacy_users(users: &mut HashMap<String, User>) -> bool {
    if users.is_empty() || users.values().any(|user| user.is_admin) {
        return false;
    }
    let Some(username) = users
        .values()
        .min_by(|a, b| {
            let created_cmp =
                normalize_created_at(a.created_at).cmp(&normalize_created_at(b.created_at));
            created_cmp.then_with(|| a.username.cmp(&b.username))
        })
        .map(|user| user.username.clone())
    else {
        return false;
    };
    if let Some(user) = users.get_mut(&username) {
        user.is_admin = true;
        return true;
    }
    false
}

fn normalize_created_at(value: i64) -> i64 {
    if value <= 0 {
        i64::MAX
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn create_user_service() -> (UserService, PathBuf) {
        let temp_dir =
            std::env::temp_dir().join(format!("reader-next-user-service-{}", random_string(8)));
        let cfg = AppConfig {
            secure: true,
            storage_dir: temp_dir.to_string_lossy().to_string(),
            ..AppConfig::default()
        };
        std::fs::create_dir_all(&temp_dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", temp_dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        (UserService::new(cfg, pool), temp_dir)
    }

    #[tokio::test]
    async fn migrates_legacy_users_json_to_sqlite_and_keeps_ai_permission() {
        let (service, temp_dir) = create_user_service().await;
        let salt = "salt1234".to_string();
        let password = gen_encrypted_password("password123", &salt);
        let token = "legacy-token".to_string();
        let users = serde_json::json!({
            "reader1": {
                "username": "reader1",
                "password": password,
                "salt": salt,
                "token": token,
                "lastLoginAt": now_ms(),
                "createdAt": 100,
                "enableAiModel": true,
                "isAdmin": false
            }
        });
        fs::create_dir_all(service.users_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&service.users_path, serde_json::to_string(&users).unwrap())
            .await
            .unwrap();

        service.migrate_legacy_users_from_json().await.unwrap();

        let access_token = "reader1:legacy-token";
        assert!(service
            .can_use_ai_model(Some(access_token), None)
            .await
            .unwrap());
        assert!(service.load_users().await.unwrap().get("reader1").is_some());
        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn login_keeps_legacy_session_and_supports_multi_device() {
        let (service, temp_dir) = create_user_service().await;

        let first_login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let first_access_token = first_login["accessToken"].as_str().unwrap().to_string();
        let (_, first_raw_token) = parse_access_token(&first_access_token).unwrap();

        let mut users = service.load_users().await.unwrap();
        let user = users.get_mut("reader1").unwrap();
        user.token = first_raw_token.clone();
        user.token_map = None;
        service.save_users(&users).await.unwrap();

        let second_login = service
            .login("reader1", "password123", true, None)
            .await
            .unwrap();
        let second_access_token = second_login["accessToken"].as_str().unwrap().to_string();
        let (_, second_raw_token) = parse_access_token(&second_access_token).unwrap();

        assert_ne!(first_access_token, second_access_token);
        assert!(service
            .check_auth(&first_access_token)
            .await
            .unwrap()
            .is_some());
        assert!(service
            .check_auth(&second_access_token)
            .await
            .unwrap()
            .is_some());

        let users = service.load_users().await.unwrap();
        let token_map = users
            .get("reader1")
            .and_then(|user| user.token_map.as_ref())
            .unwrap();
        assert!(token_map.contains_key(&first_raw_token));
        assert!(token_map.contains_key(&second_raw_token));

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn get_user_info_refreshes_current_session_and_returns_current_token() {
        let (service, temp_dir) = create_user_service().await;

        let login = service
            .login("reader2", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();
        let (_, raw_token) = parse_access_token(&access_token).unwrap();

        let short_expire_at = now_ms() + 1000;
        let mut users = service.load_users().await.unwrap();
        let user = users.get_mut("reader2").unwrap();
        user.token_map
            .as_mut()
            .unwrap()
            .insert(raw_token.clone(), short_expire_at);
        service.save_users(&users).await.unwrap();

        let (user_info, secure, secure_key_required, admin_authorized) = service
            .get_user_info(Some(&access_token), None)
            .await
            .unwrap();

        assert!(secure);
        assert!(!secure_key_required);
        assert!(admin_authorized);
        assert_eq!(
            user_info.unwrap()["accessToken"].as_str().unwrap(),
            access_token
        );

        let users = service.load_users().await.unwrap();
        let expire_at = users
            .get("reader2")
            .and_then(|user| user.token_map.as_ref())
            .and_then(|map| map.get(&raw_token))
            .copied()
            .unwrap();
        assert!(expire_at > short_expire_at);

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn get_user_info_reports_admin_authorization_from_secure_key() {
        let (mut service, temp_dir) = create_user_service().await;
        service.cfg.secure_key = "manage-key".to_string();

        let (_, secure, secure_key_required, admin_authorized) = service
            .get_user_info(None, Some("manage-key"))
            .await
            .unwrap();

        assert!(secure);
        assert!(secure_key_required);
        assert!(admin_authorized);

        let (_, _, _, denied) = service
            .get_user_info(None, Some("wrong-key"))
            .await
            .unwrap();
        assert!(!denied);

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn get_user_info_returns_logged_in_user_when_secure_is_disabled() {
        let (mut service, temp_dir) = create_user_service().await;
        let login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();
        service.cfg.secure = false;

        let (user_info, secure, _, _) = service
            .get_user_info(Some(&access_token), None)
            .await
            .unwrap();

        assert!(!secure);
        assert_eq!(user_info.unwrap()["username"].as_str().unwrap(), "reader1");

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn secure_disabled_uses_logged_in_namespace_or_default() {
        let (mut service, temp_dir) = create_user_service().await;
        let login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();
        service.cfg.secure = false;

        let logged_in_ns = service
            .resolve_user_ns_with_override(Some(&access_token), None, None)
            .await
            .unwrap();
        let anonymous_ns = service
            .resolve_user_ns_with_override(None, None, None)
            .await
            .unwrap();
        let invalid_token_ns = service
            .resolve_user_ns_with_override(Some("reader1:bad-token"), None, None)
            .await
            .unwrap();

        assert_eq!(logged_in_ns, "reader1");
        assert_eq!(anonymous_ns, "default");
        assert_eq!(invalid_token_ns, "default");

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn logged_in_namespace_wins_over_secure_key_default_fallback() {
        let (mut service, temp_dir) = create_user_service().await;
        service.cfg.secure = true;
        service.cfg.secure_key = "manage-key".to_string();

        let login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();

        let resolved = service
            .resolve_user_ns_with_override(Some(&access_token), Some("manage-key"), None)
            .await
            .unwrap();

        assert_eq!(resolved, "reader1");

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn secure_key_with_explicit_user_ns_still_overrides_namespace() {
        let (mut service, temp_dir) = create_user_service().await;
        service.cfg.secure = true;
        service.cfg.secure_key = "manage-key".to_string();

        let login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();

        let resolved = service
            .resolve_user_ns_with_override(Some(&access_token), Some("manage-key"), Some("admin"))
            .await
            .unwrap();

        assert_eq!(resolved, "admin");

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn legacy_users_without_admin_promote_earliest_account_for_ai_model_access() {
        let (service, temp_dir) = create_user_service().await;

        let first_login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let first_access_token = first_login["accessToken"].as_str().unwrap().to_string();
        let _ = service
            .login("reader2", "password123", false, None)
            .await
            .unwrap();

        let mut users = service.load_users().await.unwrap();
        users.get_mut("reader1").unwrap().is_admin = false;
        users.get_mut("reader1").unwrap().created_at = 100;
        users.get_mut("reader2").unwrap().is_admin = false;
        users.get_mut("reader2").unwrap().created_at = 200;
        service.save_users(&users).await.unwrap();

        assert!(service
            .can_use_ai_model(Some(&first_access_token), None)
            .await
            .unwrap());

        let users = service.load_users().await.unwrap();
        assert!(users.get("reader1").unwrap().is_admin);
        assert!(!users.get("reader2").unwrap().is_admin);

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn ai_model_permission_reads_legacy_snake_case_user_fields() {
        let (service, temp_dir) = create_user_service().await;
        let salt = "salt1234".to_string();
        let password = gen_encrypted_password("password123", &salt);
        let users = serde_json::json!({
            "reader1": {
                "username": "reader1",
                "password": password,
                "salt": salt,
                "token": "legacy-token",
                "last_login_at": now_ms(),
                "created_at": 100,
                "enable_ai_model": true,
                "is_admin": false
            }
        });
        fs::create_dir_all(service.users_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&service.users_path, serde_json::to_string(&users).unwrap())
            .await
            .unwrap();
        service.migrate_legacy_users_from_json().await.unwrap();

        assert!(service
            .can_use_ai_model(Some("reader1:legacy-token"), None)
            .await
            .unwrap());

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn ai_model_permission_grant_allows_existing_session_token() {
        let (service, temp_dir) = create_user_service().await;

        let login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();

        service
            .update_user("reader1", None, None, Some(true))
            .await
            .unwrap();

        assert!(service
            .can_use_ai_model(Some(&access_token), None)
            .await
            .unwrap());

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn session_refresh_does_not_overwrite_permission_changes() {
        let (service, temp_dir) = create_user_service().await;

        service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let mut stale_user = service.find_user("reader1").await.unwrap().unwrap();
        sqlx::query("UPDATE users SET enable_ai_model=1 WHERE username='reader1'")
            .execute(&service.pool)
            .await
            .unwrap();

        service.save_new_session(&mut stale_user).await.unwrap();

        let user = service.find_user("reader1").await.unwrap().unwrap();
        assert!(user.enable_ai_model);
        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn delete_user_removes_sqlite_account_documents() {
        let (service, temp_dir) = create_user_service().await;

        service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO book_sources (user_ns, book_source_url, book_source_name, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind("reader1")
        .bind("source-url")
        .bind("source-name")
        .bind("{}")
        .bind(now_ts())
        .execute(&service.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO json_documents (namespace, name, json, updated_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind("reader1")
        .bind("bookmark.json")
        .bind("[]")
        .bind(now_ts())
        .execute(&service.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind("reader1")
        .bind("book-key")
        .bind("book-url")
        .bind("{}")
        .bind(now_ts())
        .execute(&service.pool)
        .await
        .unwrap();

        service
            .delete_users(&["reader1".to_string()])
            .await
            .unwrap();

        for (sql, column) in [
            (
                "SELECT COUNT(*) AS count FROM book_sources WHERE user_ns='reader1'",
                "book_sources",
            ),
            (
                "SELECT COUNT(*) AS count FROM json_documents WHERE namespace='reader1'",
                "json_documents",
            ),
            (
                "SELECT COUNT(*) AS count FROM ai_book_memories WHERE user_ns='reader1'",
                "ai_book_memories",
            ),
        ] {
            let row = sqlx::query(sql).fetch_one(&service.pool).await.unwrap();
            assert_eq!(row.get::<i64, _>("count"), 0, "{column} should be empty");
        }

        let _ = fs::remove_dir_all(temp_dir).await;
    }

    #[tokio::test]
    async fn concurrent_auth_refreshes_keep_ai_model_permission() {
        let (service, temp_dir) = create_user_service().await;

        let _admin = service
            .login("admin", "password123", false, None)
            .await
            .unwrap();
        let login = service
            .login("reader1", "password123", false, None)
            .await
            .unwrap();
        let access_token = login["accessToken"].as_str().unwrap().to_string();
        service
            .update_user("reader1", None, None, Some(true))
            .await
            .unwrap();

        let mut handles = Vec::new();
        for _ in 0..64 {
            let service = service.clone();
            let token = access_token.clone();
            handles.push(tokio::spawn(async move {
                service.can_use_ai_model(Some(&token), None).await.unwrap()
            }));
        }
        for handle in handles {
            assert!(handle.await.unwrap());
        }

        let users = service.load_users().await.unwrap();
        let user = users.get("reader1").unwrap();
        assert!(user.enable_ai_model);
        let (_, raw_token) = parse_access_token(&access_token).unwrap();
        assert!(user
            .token_map
            .as_ref()
            .is_some_and(|token_map| token_map.contains_key(&raw_token)));

        let _ = fs::remove_dir_all(temp_dir).await;
    }
}

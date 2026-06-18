pub mod repo;

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

pub async fn init_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    let mut migrator = sqlx::migrate!("src/storage/db/migrations");
    // ponytail: old local DBs may retain migrations from reverted feature branches.
    migrator.ignore_missing = true;
    migrator.run(&pool).await?;
    Ok(pool)
}

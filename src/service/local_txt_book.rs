use crate::error::error::AppError;
use crate::model::{book::Book, book_chapter::BookChapter};
use crate::util::hash::md5_hex;
use encoding_rs::GB18030;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

pub const LOCAL_TXT_ORIGIN: &str = "local-txt";
pub const LOCAL_TXT_ORIGIN_NAME: &str = "本地 TXT";
pub const MAX_TXT_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
const LOCAL_BOOK_DIR: &str = "local_books";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedTxtChapter {
    pub title: String,
    pub url: String,
    pub index: i32,
    pub start: usize,
    pub end: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTxtChapter {
    title: String,
    url: String,
    index: i32,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTxtIndex {
    book_url: String,
    name: String,
    file_name: String,
    byte_len: usize,
    char_len: usize,
    chapters: Vec<StoredTxtChapter>,
}

pub fn is_local_txt_origin(value: &str) -> bool {
    value.trim() == LOCAL_TXT_ORIGIN
}

pub fn is_local_txt_url(value: &str) -> bool {
    value.trim().starts_with("local-txt:")
}

pub fn build_chapter_url(book_url: &str, index: usize) -> String {
    format!("{}#{}", book_url.trim_end_matches('#'), index)
}

pub fn parse_txt_chapters(book_url: &str, text: &str) -> Vec<ParsedTxtChapter> {
    let Some(heading_re) = chapter_heading_regex() else {
        return fallback_chapter(book_url, text);
    };

    let mut headings: Vec<(usize, usize, String)> = Vec::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let raw_line = line.trim_end_matches(['\r', '\n']);
        let title = raw_line.trim();
        let content_len = raw_line.len();
        if is_chapter_heading(title, &heading_re) {
            let leading_ws = raw_line.len().saturating_sub(raw_line.trim_start().len());
            headings.push((offset + leading_ws, offset + content_len, title.to_string()));
        }
        offset += line.len();
    }

    if offset < text.len() {
        let raw_line = &text[offset..];
        let title = raw_line.trim();
        if is_chapter_heading(title, &heading_re) {
            let leading_ws = raw_line.len().saturating_sub(raw_line.trim_start().len());
            headings.push((offset + leading_ws, text.len(), title.to_string()));
        }
    }

    if headings.is_empty() {
        return fallback_chapter(book_url, text);
    }

    let mut chapters = Vec::new();
    let first_start = headings[0].0;
    if text[..first_start].trim().len() > 0 {
        chapters.push(ParsedTxtChapter {
            title: "序章".to_string(),
            url: build_chapter_url(book_url, 0),
            index: 0,
            start: 0,
            end: trim_chapter_end(text, first_start),
            content: text[..trim_chapter_end(text, first_start)].to_string(),
        });
    }

    for (heading_index, (title_start, title_end, title)) in headings.iter().enumerate() {
        let content_start = skip_line_break(text, *title_end);
        let next_start = headings
            .get(heading_index + 1)
            .map(|(start, _, _)| *start)
            .unwrap_or(text.len());
        let end = trim_chapter_end(text, next_start);
        let index = chapters.len() as i32;
        chapters.push(ParsedTxtChapter {
            title: title.clone(),
            url: build_chapter_url(book_url, index as usize),
            index,
            start: content_start,
            end,
            content: text[content_start..end].to_string(),
        });
        if *title_start >= text.len() {
            break;
        }
    }

    if chapters.is_empty() {
        fallback_chapter(book_url, text)
    } else {
        chapters
    }
}

pub fn decode_txt_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(value) => value.to_string(),
        Err(_) => {
            let (decoded, _, _) = GB18030.decode(bytes);
            decoded.into_owned()
        }
    }
}

pub fn sanitize_txt_file_name(file_name: &str) -> String {
    let name = Path::new(file_name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("book.txt")
        .trim()
        .to_string();
    if name.is_empty() {
        "book.txt".to_string()
    } else {
        name
    }
}

pub fn book_name_from_file_name(file_name: &str) -> String {
    let safe = sanitize_txt_file_name(file_name);
    Path::new(&safe)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("本地小说")
        .to_string()
}

pub fn validate_txt_upload(file_name: &str, byte_len: usize) -> Result<(), AppError> {
    let safe = sanitize_txt_file_name(file_name);
    if !safe.to_lowercase().ends_with(".txt") {
        return Err(AppError::BadRequest("仅支持上传 .txt 文件".to_string()));
    }
    if byte_len == 0 {
        return Err(AppError::BadRequest("TXT 文件不能为空".to_string()));
    }
    if byte_len > MAX_TXT_UPLOAD_BYTES {
        return Err(AppError::BadRequest("TXT 文件不能超过 50MB".to_string()));
    }
    Ok(())
}

#[derive(Clone)]
pub struct LocalTxtBookService {
    storage_dir: PathBuf,
}

impl LocalTxtBookService {
    pub fn new(storage_dir: impl AsRef<Path>) -> Self {
        Self {
            storage_dir: storage_dir.as_ref().to_path_buf(),
        }
    }

    pub async fn import_txt_book(
        &self,
        user_ns: &str,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<Book, AppError> {
        validate_txt_upload(file_name, bytes.len())?;
        let safe_file_name = sanitize_txt_file_name(file_name);
        let text = decode_txt_bytes(bytes);
        if text.trim().is_empty() {
            return Err(AppError::BadRequest("TXT 文件内容不能为空".to_string()));
        }

        let hash = md5_hex(&format!(
            "{}:{}:{}",
            user_ns,
            safe_file_name,
            md5_hex(&text)
        ));
        let book_url = format!("{}:{}", LOCAL_TXT_ORIGIN, hash);
        let chapters = parse_txt_chapters(&book_url, &text);
        let book_dir = self.book_dir(user_ns, &book_url)?;
        fs::create_dir_all(&book_dir)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        fs::write(book_dir.join("book.txt"), text.as_bytes())
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        let index = StoredTxtIndex {
            book_url: book_url.clone(),
            name: book_name_from_file_name(&safe_file_name),
            file_name: safe_file_name,
            byte_len: bytes.len(),
            char_len: text.chars().count(),
            chapters: chapters
                .iter()
                .map(|chapter| StoredTxtChapter {
                    title: chapter.title.clone(),
                    url: chapter.url.clone(),
                    index: chapter.index,
                    start: chapter.start,
                    end: chapter.end,
                })
                .collect(),
        };
        let data =
            serde_json::to_string_pretty(&index).map_err(|e| AppError::Internal(e.into()))?;
        fs::write(book_dir.join("chapters.json"), data)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

        Ok(Book {
            name: index.name,
            author: "本地导入".to_string(),
            book_url: book_url.clone(),
            origin: LOCAL_TXT_ORIGIN.to_string(),
            origin_name: Some(LOCAL_TXT_ORIGIN_NAME.to_string()),
            toc_url: Some(book_url),
            can_update: Some(false),
            dur_chapter_index: Some(0),
            dur_chapter_pos: Some(0),
            total_chapter_num: Some(index.chapters.len() as i32),
            latest_chapter_title: index.chapters.last().map(|chapter| chapter.title.clone()),
            kind: Some("本地TXT".to_string()),
            word_count: Some(format!("{}字", index.char_len)),
            ..Book::default()
        })
    }

    pub async fn get_book_info(&self, user_ns: &str, book_url: &str) -> Result<Book, AppError> {
        let index = self.read_index(user_ns, book_url).await?;
        Ok(Book {
            name: index.name,
            author: "本地导入".to_string(),
            book_url: index.book_url.clone(),
            origin: LOCAL_TXT_ORIGIN.to_string(),
            origin_name: Some(LOCAL_TXT_ORIGIN_NAME.to_string()),
            toc_url: Some(index.book_url.clone()),
            can_update: Some(false),
            total_chapter_num: Some(index.chapters.len() as i32),
            latest_chapter_title: index.chapters.last().map(|chapter| chapter.title.clone()),
            kind: Some("本地TXT".to_string()),
            word_count: Some(format!("{}字", index.char_len)),
            ..Book::default()
        })
    }

    pub async fn get_chapter_list(
        &self,
        user_ns: &str,
        book_url: &str,
    ) -> Result<Vec<BookChapter>, AppError> {
        let index = self.read_index(user_ns, book_url).await?;
        Ok(index
            .chapters
            .into_iter()
            .map(|chapter| BookChapter {
                title: chapter.title,
                url: chapter.url,
                index: chapter.index,
                ..BookChapter::default()
            })
            .collect())
    }

    pub async fn get_content(&self, user_ns: &str, chapter_url: &str) -> Result<String, AppError> {
        let (book_url, requested_index) = parse_chapter_url(chapter_url)?;
        let index = self.read_index(user_ns, &book_url).await?;
        let chapter = index
            .chapters
            .iter()
            .find(|chapter| chapter.index == requested_index)
            .ok_or_else(|| AppError::BadRequest("章节不存在".to_string()))?;
        let text = fs::read_to_string(self.book_dir(user_ns, &book_url)?.join("book.txt"))
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        if chapter.start > chapter.end || chapter.end > text.len() {
            return Err(AppError::BadRequest("章节索引无效".to_string()));
        }
        Ok(text[chapter.start..chapter.end].to_string())
    }

    fn local_root(&self, user_ns: &str) -> PathBuf {
        self.storage_dir
            .join("data")
            .join(user_ns)
            .join(LOCAL_BOOK_DIR)
    }

    fn book_dir(&self, user_ns: &str, book_url: &str) -> Result<PathBuf, AppError> {
        let hash = book_url
            .strip_prefix("local-txt:")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::BadRequest("本地 TXT 地址无效".to_string()))?;
        Ok(self.local_root(user_ns).join(hash))
    }

    async fn read_index(&self, user_ns: &str, book_url: &str) -> Result<StoredTxtIndex, AppError> {
        let path = self.book_dir(user_ns, book_url)?.join("chapters.json");
        let data = fs::read_to_string(path)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        serde_json::from_str(&data).map_err(|e| AppError::BadRequest(e.to_string()))
    }
}

fn fallback_chapter(book_url: &str, text: &str) -> Vec<ParsedTxtChapter> {
    vec![ParsedTxtChapter {
        title: "正文".to_string(),
        url: build_chapter_url(book_url, 0),
        index: 0,
        start: 0,
        end: text.len(),
        content: text.to_string(),
    }]
}

fn chapter_heading_regex() -> Option<Regex> {
    Regex::new(r"^(?:第[\p{N}零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+[章节回卷部集篇]|卷[\p{N}零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+)\b.*$").ok()
}

fn is_chapter_heading(line: &str, re: &Regex) -> bool {
    if line.is_empty() || line.chars().count() > 80 {
        return false;
    }
    re.is_match(line)
}

fn skip_line_break(text: &str, offset: usize) -> usize {
    let rest = &text[offset..];
    if rest.starts_with("\r\n") {
        offset + 2
    } else if rest.starts_with('\n') || rest.starts_with('\r') {
        offset + 1
    } else {
        offset
    }
}

fn trim_chapter_end(text: &str, mut end: usize) -> usize {
    while end > 0 {
        let prev = text[..end].chars().next_back();
        match prev {
            Some('\n') | Some('\r') => end -= prev.unwrap().len_utf8(),
            _ => break,
        }
    }
    end
}

fn parse_chapter_url(chapter_url: &str) -> Result<(String, i32), AppError> {
    let (book_url, raw_index) = chapter_url
        .rsplit_once('#')
        .ok_or_else(|| AppError::BadRequest("章节地址无效".to_string()))?;
    if !is_local_txt_url(book_url) {
        return Err(AppError::BadRequest("章节地址无效".to_string()));
    }
    let index = raw_index
        .parse::<i32>()
        .map_err(|_| AppError::BadRequest("章节序号无效".to_string()))?;
    Ok((book_url.to_string(), index))
}

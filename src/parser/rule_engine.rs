use crate::model::rule::{BookInfoRule, SearchRule, TocRule};
use crate::model::{
    book::Book, book_chapter::BookChapter, book_source::BookSource, search::SearchBook,
};
use crate::parser::{
    html,
    js::{eval_js, eval_js_with_bindings, with_js_lib},
    jsonpath,
};
use crate::util::text::{apply_regex_replace, normalize_source_url};
use serde_json::{json, Value};
use std::collections::HashMap;
use sxd_xpath::{Context as XPathContext, Factory as XPathFactory, Value as XPathValue};

#[derive(Clone, Default)]
pub struct RuleEngine;

#[derive(Debug, Clone, PartialEq)]
enum ParseMode {
    Css,      // CSS selector
    XPath,    // XPath expression
    JsonPath, // JSONPath expression
    Regex,    // Regex pattern
    Js,       // JavaScript
}

impl RuleEngine {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Detect the parsing mode from the rule string
    fn detect_mode(&self, rule: &str, content: &str) -> ParseMode {
        let rule = rule.trim();

        // Explicit mode forcing
        if rule.starts_with("@css:") || rule.starts_with("@CSS:") {
            return ParseMode::Css;
        }
        if rule.starts_with("@xpath:") || rule.starts_with("@XPath:") || rule.starts_with("@XPATH:")
        {
            return ParseMode::XPath;
        }
        if rule.starts_with("@json:") || rule.starts_with("@Json:") || rule.starts_with("@JSON:") {
            return ParseMode::JsonPath;
        }
        if rule.starts_with("@regex:") || rule.starts_with("@Regex:") {
            return ParseMode::Regex;
        }
        if rule.starts_with("js:") || rule.starts_with("@js:") || rule.starts_with("<js>") {
            return ParseMode::Js;
        }

        // Auto-detect from rule prefix
        if rule.starts_with('/') || rule.starts_with("./") {
            return ParseMode::XPath;
        }
        if rule.starts_with("$.") || rule.starts_with("$[") {
            return ParseMode::JsonPath;
        }
        if rule.starts_with(':') {
            return ParseMode::Regex;
        }

        // Auto-detect from content
        let content_trimmed = content.trim();
        if content_trimmed.starts_with('{') || content_trimmed.starts_with('[') {
            // Likely JSON content
            if rule.starts_with("$.") || rule.starts_with("$[") {
                return ParseMode::JsonPath;
            }
            // Try to parse as JSON
            if serde_json::from_str::<Value>(content_trimmed).is_ok() {
                return ParseMode::JsonPath;
            }
        }

        // Default to CSS
        ParseMode::Css
    }

    /// Strip mode prefix from rule
    fn strip_mode_prefix<'a>(&self, rule: &'a str) -> &'a str {
        strip_mode_prefix(rule)
    }

    pub fn search_books(&self, source: &BookSource, body: &str, base_url: &str) -> Vec<SearchBook> {
        with_js_lib(source.js_lib.as_deref(), || {
            let rule = source.rule_search.clone().unwrap_or_default();
            let (list_rule, reverse) = normalize_list_rule(rule.book_list.as_deref().unwrap_or(""));
            let mode = self.detect_mode(list_rule, body);
            let mut results = match mode {
                ParseMode::JsonPath => {
                    self.search_books_json(source, body, base_url, &rule, list_rule)
                }
                ParseMode::XPath => {
                    self.search_books_xpath(source, body, base_url, &rule, list_rule)
                }
                ParseMode::Js => self.search_books_js(source, body, base_url, &rule, list_rule),
                ParseMode::Regex => {
                    self.search_books_regex(source, body, base_url, &rule, list_rule)
                }
                ParseMode::Css => self.search_books_html(source, body, base_url, &rule, list_rule),
            };

            if results.is_empty()
                && source
                    .book_url_pattern
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            {
                if let Some(detail_book) = self.search_detail_fallback(source, body, base_url) {
                    results.push(detail_book);
                }
            }
            if reverse {
                results.reverse();
            }
            results
        })
    }

    pub fn explore_books(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
    ) -> Vec<SearchBook> {
        with_js_lib(source.js_lib.as_deref(), || {
            let rule = source
                .rule_explore
                .clone()
                .filter(|rule| {
                    rule.book_list
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty())
                })
                .unwrap_or_else(|| source.rule_search.clone().unwrap_or_default());
            let (list_rule, reverse) = normalize_list_rule(rule.book_list.as_deref().unwrap_or(""));
            let mode = self.detect_mode(list_rule, body);
            let mut results = match mode {
                ParseMode::JsonPath => {
                    self.search_books_json(source, body, base_url, &rule, list_rule)
                }
                ParseMode::XPath => {
                    self.search_books_xpath(source, body, base_url, &rule, list_rule)
                }
                ParseMode::Js => self.search_books_js(source, body, base_url, &rule, list_rule),
                ParseMode::Regex => {
                    self.search_books_regex(source, body, base_url, &rule, list_rule)
                }
                ParseMode::Css => self.search_books_html(source, body, base_url, &rule, list_rule),
            };
            if reverse {
                results.reverse();
            }
            results
        })
    }

    pub fn book_info(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
        book_url: &str,
    ) -> Book {
        with_js_lib(source.js_lib.as_deref(), || {
            let rule = source.rule_book_info.clone().unwrap_or_default();
            let mut context = HashMap::new();

            let mode = self.detect_mode(rule.name.as_deref().unwrap_or(""), body);
            match mode {
                ParseMode::JsonPath => {
                    if let Ok(v) = serde_json::from_str::<Value>(body) {
                        return parse_book_info_json(
                            source,
                            &v,
                            base_url,
                            &rule,
                            book_url,
                            &mut context,
                        );
                    }
                }
                ParseMode::XPath => {
                    return parse_book_info_xpath(
                        source,
                        body,
                        base_url,
                        &rule,
                        book_url,
                        &mut context,
                    );
                }
                _ => {}
            }
            parse_book_info_html(source, body, base_url, &rule, book_url, &mut context)
        })
    }

    pub fn chapter_list(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
    ) -> (Vec<BookChapter>, Vec<String>) {
        with_js_lib(source.js_lib.as_deref(), || {
            let rule = source.rule_toc.clone().unwrap_or_default();
            let mut context = HashMap::new();
            let (list_rule, reverse) =
                normalize_list_rule(rule.chapter_list.as_deref().unwrap_or(""));
            let prepared_body = prepare_toc_body(body, base_url, &rule);
            let mode = self.detect_mode(list_rule, &prepared_body);
            let (mut chapters, next_urls) = match mode {
                ParseMode::JsonPath => parse_chapter_list_json(
                    &prepared_body,
                    base_url,
                    &rule,
                    list_rule,
                    &mut context,
                ),
                ParseMode::XPath => parse_chapter_list_xpath(
                    &prepared_body,
                    base_url,
                    &rule,
                    list_rule,
                    &mut context,
                ),
                ParseMode::Js => self.parse_chapter_list_js(
                    &prepared_body,
                    base_url,
                    &rule,
                    list_rule,
                    &mut context,
                ),
                ParseMode::Regex => {
                    self.parse_chapter_list_regex(&prepared_body, base_url, &rule, list_rule)
                }
                ParseMode::Css => parse_chapter_list_html(
                    &prepared_body,
                    base_url,
                    &rule,
                    list_rule,
                    &mut context,
                ),
            };
            apply_toc_format_js(&mut chapters, rule.format_js.as_deref(), base_url);
            if reverse {
                chapters.reverse();
            }
            for (index, chapter) in chapters.iter_mut().enumerate() {
                chapter.index = index as i32;
            }
            (chapters, next_urls)
        })
    }

    pub fn content(&self, source: &BookSource, body: &str, base_url: &str) -> String {
        with_js_lib(source.js_lib.as_deref(), || {
            let rule = source.rule_content.clone().unwrap_or_default();
            let mut content_body = body.to_string();

            if let Some(source_regex) = rule
                .source_regex
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                content_body = apply_legado_regex(&content_body, source_regex);
            }
            if let Some(web_js) = rule.web_js.as_deref().filter(|s| !s.trim().is_empty()) {
                if let Ok(processed) =
                    eval_js(self.strip_mode_prefix(web_js), &content_body, base_url)
                {
                    if !processed.trim().is_empty() {
                        content_body = processed;
                    }
                }
            }

            if let Some(content_rule) = rule.content.clone() {
                if matches!(
                    self.detect_mode(&content_rule, &content_body),
                    ParseMode::Js
                ) {
                    let script = self.strip_mode_prefix(&content_rule);
                    if let Ok(res) = eval_js(script, &content_body, base_url) {
                        return res;
                    }
                }

                let content_rule = self.process_inline_js(&content_rule, &content_body, base_url);

                let mode = self.detect_mode(&content_rule, &content_body);
                let mut content = match mode {
                    ParseMode::JsonPath => {
                        if let Ok(v) = serde_json::from_str::<Value>(&content_body) {
                            jsonpath::jsonpath_first_string(
                                &v,
                                self.strip_mode_prefix(&content_rule),
                            )
                            .unwrap_or_default()
                        } else {
                            String::new()
                        }
                    }
                    ParseMode::XPath => {
                        html::select_xpath(&content_body, self.strip_mode_prefix(&content_rule))
                            .first()
                            .cloned()
                            .unwrap_or_default()
                    }
                    _ => {
                        let doc = html::parse_document(&content_body);
                        let result =
                            html::select_all_text(&doc, self.strip_mode_prefix(&content_rule));
                        result.unwrap_or_default()
                    }
                };

                if let Some(replace) = rule.replace_regex.as_deref() {
                    content = apply_legado_regex(&content, replace);
                }

                return content;
            }

            String::new()
        })
    }

    /// Process inline JavaScript {{...}} in rules
    fn process_inline_js(&self, rule: &str, body: &str, base_url: &str) -> String {
        let mut result = rule.to_string();

        // Find all {{...}} blocks and evaluate them
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        for cap in re.captures_iter(rule) {
            if let Some(js_code) = cap.get(1) {
                if let Ok(js_result) = eval_js(js_code.as_str(), body, base_url) {
                    result = result.replace(cap.get(0).unwrap().as_str(), &js_result);
                }
            }
        }

        result
    }

    /// Get the next content page URL if pagination exists
    pub fn next_content_url(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
    ) -> Option<String> {
        let rule = source.rule_content.clone().unwrap_or_default();
        let next_rule = rule.next_content_url.as_deref()?;
        if next_rule.is_empty() {
            return None;
        }

        let mode = self.detect_mode(next_rule, body);
        let next_url = match mode {
            ParseMode::JsonPath => {
                if let Ok(v) = serde_json::from_str::<Value>(body) {
                    jsonpath::jsonpath_first_string(&v, self.strip_mode_prefix(next_rule))
                } else {
                    None
                }
            }
            ParseMode::XPath => html::select_xpath(body, self.strip_mode_prefix(next_rule))
                .first()
                .cloned(),
            _ => {
                let doc = html::parse_document(body);
                html::select_text(&doc, self.strip_mode_prefix(next_rule))
            }
        };

        if next_url.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            return None;
        }

        let next_url = next_url?;
        Some(resolve_url(base_url, &next_url))
    }

    fn search_detail_fallback(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
    ) -> Option<SearchBook> {
        let book = self.book_info(source, body, base_url, base_url);
        search_book_from_book(book)
    }

    fn search_books_js(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
        rule: &SearchRule,
        list_rule: &str,
    ) -> Vec<SearchBook> {
        let output = match eval_js(self.strip_mode_prefix(list_rule), body, base_url) {
            Ok(result) => result,
            Err(_) => return vec![],
        };

        if let Some(items) = parse_js_output_items(&output) {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Some(book) = build_search_book_from_json(source, &item, base_url, rule) {
                    out.push(book);
                }
            }
            return out;
        }

        let doc = html::parse_document(&output);
        let sel = match scraper::Selector::parse("body > *") {
            Ok(sel) => sel,
            Err(_) => return vec![],
        };
        let mut out = Vec::new();
        for el in doc.select(&sel) {
            let name = rule
                .name
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let author = rule
                .author
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .unwrap_or_default();
            let book_url = rule
                .book_url
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .unwrap_or_default();
            let cover_url = rule
                .cover_url
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .map(|u| resolve_url(base_url, &u));
            let intro = rule
                .intro
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let kind = rule
                .kind
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let last_chapter = rule
                .last_chapter
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let update_time = rule
                .update_time
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let word_count = rule
                .word_count
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            out.push(SearchBook {
                name,
                author,
                book_url: resolve_url(base_url, &book_url),
                origin: source.book_source_url.clone(),
                cover_url,
                intro,
                kind,
                last_chapter: clean_last_chapter(last_chapter),
                update_time,
                word_count,
                book_source_urls: None,
            });
        }
        out
    }

    fn search_books_regex(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
        rule: &SearchRule,
        list_rule: &str,
    ) -> Vec<SearchBook> {
        let pattern = self
            .strip_mode_prefix(list_rule)
            .trim_start_matches(':')
            .trim();
        let re = match regex::Regex::new(pattern) {
            Ok(re) => re,
            Err(_) => return vec![],
        };

        let mut out = Vec::new();
        for captures in re.captures_iter(body) {
            let name = capture_rule_value(rule.name.as_deref(), &captures).unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let author = capture_rule_value(rule.author.as_deref(), &captures).unwrap_or_default();
            let book_url =
                capture_rule_value(rule.book_url.as_deref(), &captures).unwrap_or_default();
            let cover_url = capture_rule_value(rule.cover_url.as_deref(), &captures)
                .map(|u| resolve_url(base_url, &u));
            let intro = capture_rule_value(rule.intro.as_deref(), &captures);
            let kind = capture_rule_value(rule.kind.as_deref(), &captures);
            let last_chapter = capture_rule_value(rule.last_chapter.as_deref(), &captures);
            let update_time = capture_rule_value(rule.update_time.as_deref(), &captures);
            let word_count = capture_rule_value(rule.word_count.as_deref(), &captures);
            out.push(SearchBook {
                name,
                author,
                book_url: resolve_url(base_url, &book_url),
                origin: source.book_source_url.clone(),
                cover_url,
                intro,
                kind,
                last_chapter: clean_last_chapter(last_chapter),
                update_time,
                word_count,
                book_source_urls: None,
            });
        }
        out
    }

    fn parse_chapter_list_js(
        &self,
        body: &str,
        base_url: &str,
        rule: &TocRule,
        list_rule: &str,
        ctx: &mut HashMap<String, String>,
    ) -> (Vec<BookChapter>, Vec<String>) {
        let output = match eval_js(self.strip_mode_prefix(list_rule), body, base_url) {
            Ok(result) => result,
            Err(_) => return (vec![], vec![]),
        };

        if let Some(items) = parse_js_output_items(&output) {
            let mut out = Vec::with_capacity(items.len());
            let mut seen_urls = std::collections::HashSet::new();
            for item in items {
                if let Some(chapter) =
                    build_chapter_from_json(&item, base_url, rule, ctx, out.len())
                {
                    if seen_urls.insert(chapter.url.clone()) {
                        out.push(chapter);
                    }
                }
            }
            return (out, vec![]);
        }

        let doc = html::parse_document(&output);
        let sel = match scraper::Selector::parse("body > *") {
            Ok(sel) => sel,
            Err(_) => return (vec![], vec![]),
        };
        let mut out = Vec::new();
        let mut seen_urls = std::collections::HashSet::new();
        for el in doc.select(&sel) {
            let title = rule
                .chapter_name
                .as_ref()
                .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
                .unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let raw_url = rule
                .chapter_url
                .as_ref()
                .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
                .unwrap_or_default();
            let tag = rule
                .update_time
                .as_ref()
                .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx));
            let is_volume = rule
                .is_volume
                .as_ref()
                .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
                .map(is_truthy)
                .unwrap_or(false);
            let is_vip = rule
                .is_vip
                .as_ref()
                .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
                .map(is_truthy)
                .unwrap_or(false);
            let is_pay = rule
                .is_pay
                .as_ref()
                .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
                .map(is_truthy)
                .unwrap_or(false);
            let url = finalize_chapter_url(base_url, &raw_url, &title, is_volume, out.len());
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            out.push(BookChapter {
                title,
                url,
                index: out.len() as i32,
                tag,
                is_vip,
                is_pay,
                is_volume,
                ..Default::default()
            });
        }
        (out, vec![])
    }

    fn parse_chapter_list_regex(
        &self,
        body: &str,
        base_url: &str,
        rule: &TocRule,
        list_rule: &str,
    ) -> (Vec<BookChapter>, Vec<String>) {
        let pattern = self
            .strip_mode_prefix(list_rule)
            .trim_start_matches(':')
            .trim();
        let re = match regex::Regex::new(pattern) {
            Ok(re) => re,
            Err(_) => return (vec![], vec![]),
        };

        let mut out = Vec::new();
        let mut seen_urls = std::collections::HashSet::new();
        for captures in re.captures_iter(body) {
            let title =
                capture_rule_value(rule.chapter_name.as_deref(), &captures).unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let raw_url =
                capture_rule_value(rule.chapter_url.as_deref(), &captures).unwrap_or_default();
            let tag = capture_rule_value(rule.update_time.as_deref(), &captures);
            let is_volume = capture_rule_value(rule.is_volume.as_deref(), &captures)
                .map(is_truthy)
                .unwrap_or(false);
            let is_vip = capture_rule_value(rule.is_vip.as_deref(), &captures)
                .map(is_truthy)
                .unwrap_or(false);
            let is_pay = capture_rule_value(rule.is_pay.as_deref(), &captures)
                .map(is_truthy)
                .unwrap_or(false);
            let url = finalize_chapter_url(base_url, &raw_url, &title, is_volume, out.len());
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            out.push(BookChapter {
                title,
                url,
                index: out.len() as i32,
                tag,
                is_vip,
                is_pay,
                is_volume,
                ..Default::default()
            });
        }
        (out, vec![])
    }

    fn search_books_html(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
        rule: &SearchRule,
        list_sel: &str,
    ) -> Vec<SearchBook> {
        if list_sel.trim().is_empty() {
            return vec![];
        }
        let doc = html::parse_document(body);
        let items = html::select_list(&doc, self.strip_mode_prefix(list_sel));
        let mut out = Vec::with_capacity(items.len());

        for el in items {
            let name = rule
                .name
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .unwrap_or_default();
            let author = rule
                .author
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .unwrap_or_default();
            let book_url = rule
                .book_url
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url))
                .unwrap_or_default();
            let cover_url = rule
                .cover_url
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let intro = rule
                .intro
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let kind = rule
                .kind
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let last_chapter = rule
                .last_chapter
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let update_time = rule
                .update_time
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let word_count = rule
                .word_count
                .as_ref()
                .and_then(|r| eval_field_html(r, &el, base_url));
            let book_url_abs = resolve_url(base_url, &book_url);
            let cover_url_abs = cover_url.map(|u| resolve_url(base_url, &u));
            out.push(SearchBook {
                name,
                author,
                book_url: book_url_abs,
                origin: source.book_source_url.clone(),
                cover_url: cover_url_abs,
                intro,
                kind,
                last_chapter: clean_last_chapter(last_chapter),
                update_time,
                word_count,
                book_source_urls: None,
            });
        }
        out
    }

    fn search_books_xpath(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
        rule: &SearchRule,
        list_rule: &str,
    ) -> Vec<SearchBook> {
        let package = match sxd_document::parser::parse(body) {
            Ok(p) => p,
            Err(_) => return vec![],
        };
        let document = package.as_document();
        let items = xpath_select_nodes(
            sxd_xpath::nodeset::Node::Root(document.root()),
            self.strip_mode_prefix(list_rule),
        );
        let mut out = Vec::with_capacity(items.len());

        for item in items {
            let name = eval_field_xpath(rule.name.as_deref().unwrap_or(""), item, base_url);
            let author = eval_field_xpath(rule.author.as_deref().unwrap_or(""), item, base_url);
            let book_url = eval_field_xpath(rule.book_url.as_deref().unwrap_or(""), item, base_url);
            let cover_url =
                eval_field_xpath(rule.cover_url.as_deref().unwrap_or(""), item, base_url);
            let intro = eval_field_xpath(rule.intro.as_deref().unwrap_or(""), item, base_url);
            let kind = eval_field_xpath(rule.kind.as_deref().unwrap_or(""), item, base_url);
            let last_chapter =
                eval_field_xpath(rule.last_chapter.as_deref().unwrap_or(""), item, base_url);
            let update_time =
                eval_field_xpath(rule.update_time.as_deref().unwrap_or(""), item, base_url);
            let word_count =
                eval_field_xpath(rule.word_count.as_deref().unwrap_or(""), item, base_url);
            out.push(SearchBook {
                name: name.unwrap_or_default(),
                author: author.unwrap_or_default(),
                book_url: resolve_url(base_url, &book_url.unwrap_or_default()),
                origin: source.book_source_url.clone(),
                cover_url: cover_url.map(|u| resolve_url(base_url, &u)),
                intro,
                kind,
                last_chapter: clean_last_chapter(last_chapter),
                update_time,
                word_count,
                book_source_urls: None,
            });
        }

        out
    }

    fn search_books_json(
        &self,
        source: &BookSource,
        body: &str,
        base_url: &str,
        rule: &SearchRule,
        list_rule: &str,
    ) -> Vec<SearchBook> {
        let v: Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let items = jsonpath::jsonpath_query(&v, self.strip_mode_prefix(list_rule));
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let name = eval_field_json(rule.name.as_deref().unwrap_or(""), &item, base_url);
            let author = eval_field_json(rule.author.as_deref().unwrap_or(""), &item, base_url);
            let book_url = eval_field_json(rule.book_url.as_deref().unwrap_or(""), &item, base_url);
            let cover_url =
                eval_field_json(rule.cover_url.as_deref().unwrap_or(""), &item, base_url);
            let intro = eval_field_json(rule.intro.as_deref().unwrap_or(""), &item, base_url);
            let kind = eval_field_json(rule.kind.as_deref().unwrap_or(""), &item, base_url);
            let last_chapter =
                eval_field_json(rule.last_chapter.as_deref().unwrap_or(""), &item, base_url);
            let update_time =
                eval_field_json(rule.update_time.as_deref().unwrap_or(""), &item, base_url);
            let word_count =
                eval_field_json(rule.word_count.as_deref().unwrap_or(""), &item, base_url);
            let book_url = book_url.map(|u| resolve_url(base_url, &u));
            let cover_url = cover_url.map(|u| resolve_url(base_url, &u));
            out.push(SearchBook {
                name: name.unwrap_or_default(),
                author: author.unwrap_or_default(),
                book_url: book_url.unwrap_or_default(),
                origin: source.book_source_url.clone(),
                cover_url,
                intro,
                kind,
                last_chapter: clean_last_chapter(last_chapter),
                update_time,
                word_count,
                book_source_urls: None,
            });
        }
        out
    }
}

fn clean_last_chapter(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    if value.chars().count() <= 80 && value.lines().count() <= 1 {
        return Some(value);
    }
    value
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('第') && line.contains('章'))
        .last()
        .map(str::to_string)
        .or(Some(value))
}

fn parse_book_info_html(
    source: &BookSource,
    body: &str,
    base_url: &str,
    rule: &BookInfoRule,
    book_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Book {
    let doc = html::parse_document(body);

    // Execute init rule if present
    if let Some(init) = &rule.init {
        let _ = eval_field_html_doc_with_ctx(init, &doc, base_url, ctx);
    }

    let name = rule
        .name
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx))
        .unwrap_or_default();
    let author = rule
        .author
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx))
        .unwrap_or_default();
    let intro = rule
        .intro
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));
    let kind = rule
        .kind
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));
    let last_chapter = rule
        .last_chapter
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));
    let update_time = rule
        .update_time
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));
    let cover_url = rule
        .cover_url
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx))
        .map(|u| resolve_url(base_url, &u));
    let word_count = rule
        .word_count
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));
    let toc_url = rule
        .toc_url
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx))
        .map(|u| resolve_url(base_url, &u));
    let can_re_name = rule
        .can_re_name
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));
    let download_urls = rule
        .download_urls
        .as_ref()
        .and_then(|r| eval_field_html_doc_with_ctx(r, &doc, base_url, ctx));

    let final_toc_url = toc_url.or_else(|| Some(book_url.to_string()));

    Book {
        name,
        author,
        book_url: book_url.to_string(),
        origin: source.book_source_url.clone(),
        origin_name: Some(source.book_source_name.clone()),
        cover_url,
        toc_url: final_toc_url,
        intro,
        latest_chapter_title: clean_last_chapter(last_chapter),
        word_count,
        info_html: None,
        toc_html: None,
        kind,
        update_time,
        can_re_name,
        download_urls,
        ..Default::default()
    }
}

fn parse_book_info_xpath(
    source: &BookSource,
    body: &str,
    base_url: &str,
    rule: &BookInfoRule,
    book_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Book {
    let package = match sxd_document::parser::parse(body) {
        Ok(p) => p,
        Err(_) => return parse_book_info_html(source, body, base_url, rule, book_url, ctx),
    };
    let document = package.as_document();
    let scope = select_xpath_scope(
        sxd_xpath::nodeset::Node::Root(document.root()),
        rule.init.as_deref(),
    );

    let name = eval_field_xpath_with_ctx(rule.name.as_deref().unwrap_or(""), scope, base_url, ctx)
        .unwrap_or_default();
    let author =
        eval_field_xpath_with_ctx(rule.author.as_deref().unwrap_or(""), scope, base_url, ctx)
            .unwrap_or_default();
    let intro =
        eval_field_xpath_with_ctx(rule.intro.as_deref().unwrap_or(""), scope, base_url, ctx);
    let kind = eval_field_xpath_with_ctx(rule.kind.as_deref().unwrap_or(""), scope, base_url, ctx);
    let last_chapter = eval_field_xpath_with_ctx(
        rule.last_chapter.as_deref().unwrap_or(""),
        scope,
        base_url,
        ctx,
    );
    let update_time = eval_field_xpath_with_ctx(
        rule.update_time.as_deref().unwrap_or(""),
        scope,
        base_url,
        ctx,
    );
    let cover_url = eval_field_xpath_with_ctx(
        rule.cover_url.as_deref().unwrap_or(""),
        scope,
        base_url,
        ctx,
    )
    .map(|u| resolve_url(base_url, &u));
    let word_count = eval_field_xpath_with_ctx(
        rule.word_count.as_deref().unwrap_or(""),
        scope,
        base_url,
        ctx,
    );
    let toc_url =
        eval_field_xpath_with_ctx(rule.toc_url.as_deref().unwrap_or(""), scope, base_url, ctx)
            .map(|u| resolve_url(base_url, &u));
    let can_re_name = eval_field_xpath_with_ctx(
        rule.can_re_name.as_deref().unwrap_or(""),
        scope,
        base_url,
        ctx,
    );
    let download_urls = eval_field_xpath_with_ctx(
        rule.download_urls.as_deref().unwrap_or(""),
        scope,
        base_url,
        ctx,
    );

    Book {
        name,
        author,
        book_url: book_url.to_string(),
        origin: source.book_source_url.clone(),
        origin_name: Some(source.book_source_name.clone()),
        cover_url,
        toc_url: toc_url.or_else(|| Some(book_url.to_string())),
        intro,
        latest_chapter_title: clean_last_chapter(last_chapter),
        word_count,
        info_html: None,
        toc_html: None,
        kind,
        update_time,
        can_re_name,
        download_urls,
        ..Default::default()
    }
}

fn parse_book_info_json(
    source: &BookSource,
    v: &Value,
    base_url: &str,
    rule: &BookInfoRule,
    book_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Book {
    let scope = select_json_scope(v, rule.init.as_deref(), base_url, ctx);
    let name = eval_field_json_with_ctx(rule.name.as_deref().unwrap_or(""), &scope, base_url, ctx)
        .unwrap_or_default();
    let author =
        eval_field_json_with_ctx(rule.author.as_deref().unwrap_or(""), &scope, base_url, ctx)
            .unwrap_or_default();
    let intro =
        eval_field_json_with_ctx(rule.intro.as_deref().unwrap_or(""), &scope, base_url, ctx);
    let kind = eval_field_json_with_ctx(rule.kind.as_deref().unwrap_or(""), &scope, base_url, ctx);
    let last_chapter = eval_field_json_with_ctx(
        rule.last_chapter.as_deref().unwrap_or(""),
        &scope,
        base_url,
        ctx,
    );
    let update_time = eval_field_json_with_ctx(
        rule.update_time.as_deref().unwrap_or(""),
        &scope,
        base_url,
        ctx,
    );
    let cover_url = eval_field_json_with_ctx(
        rule.cover_url.as_deref().unwrap_or(""),
        &scope,
        base_url,
        ctx,
    )
    .map(|u| resolve_url(base_url, &u));
    let word_count = eval_field_json_with_ctx(
        rule.word_count.as_deref().unwrap_or(""),
        &scope,
        base_url,
        ctx,
    );
    let toc_url =
        eval_field_json_with_ctx(rule.toc_url.as_deref().unwrap_or(""), &scope, base_url, ctx)
            .map(|u| resolve_url(base_url, &u));
    let can_re_name = eval_field_json_with_ctx(
        rule.can_re_name.as_deref().unwrap_or(""),
        &scope,
        base_url,
        ctx,
    );
    let download_urls = eval_field_json_with_ctx(
        rule.download_urls.as_deref().unwrap_or(""),
        &scope,
        base_url,
        ctx,
    );
    Book {
        name,
        author,
        book_url: book_url.to_string(),
        origin: source.book_source_url.clone(),
        origin_name: Some(source.book_source_name.clone()),
        cover_url,
        toc_url: toc_url.or_else(|| Some(book_url.to_string())),
        intro,
        latest_chapter_title: clean_last_chapter(last_chapter),
        word_count,
        info_html: None,
        toc_html: None,
        kind,
        update_time,
        can_re_name,
        download_urls,
        ..Default::default()
    }
}

fn parse_chapter_list_html(
    body: &str,
    base_url: &str,
    rule: &TocRule,
    list_sel: &str,
    ctx: &mut HashMap<String, String>,
) -> (Vec<BookChapter>, Vec<String>) {
    if list_sel.trim().is_empty() {
        return (vec![], vec![]);
    }
    let doc = html::parse_document(body);

    // Execute init rule if present
    if let Some(init) = &rule.init {
        let _ = eval_field_html_doc_with_ctx(init, &doc, base_url, ctx);
    }

    let items = html::select_list(&doc, strip_mode_prefix(list_sel));

    // Use a set to deduplicate chapters by URL
    let mut seen_urls = std::collections::HashSet::new();
    let mut out: Vec<BookChapter> = Vec::with_capacity(items.len());

    for el in items {
        let title = rule
            .chapter_name
            .as_ref()
            .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
            .unwrap_or_default();
        let url = rule
            .chapter_url
            .as_ref()
            .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
            .unwrap_or_default();
        let tag = rule
            .update_time
            .as_ref()
            .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx));
        let is_volume = rule
            .is_volume
            .as_ref()
            .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
            .map(is_truthy)
            .unwrap_or(false);
        let is_vip = rule
            .is_vip
            .as_ref()
            .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
            .map(is_truthy)
            .unwrap_or(false);
        let is_pay = rule
            .is_pay
            .as_ref()
            .and_then(|r| eval_field_html_with_ctx(r, &el, base_url, ctx))
            .map(is_truthy)
            .unwrap_or(false);
        let url_abs = finalize_chapter_url(base_url, &url, &title, is_volume, out.len());

        if seen_urls.contains(&url_abs) {
            out.retain(|chapter| chapter.url != url_abs);
        } else {
            seen_urls.insert(url_abs.clone());
        }

        out.push(BookChapter {
            title,
            url: url_abs,
            index: out.len() as i32,
            tag,
            is_vip,
            is_pay,
            is_volume,
            ..Default::default()
        });
    }

    // Extract next_toc_url(s)
    let rule_str = rule.next_toc_url.as_deref().unwrap_or("");
    let raw_urls: Vec<String> = html::select_text_list(&doc, rule_str);
    let next_urls: Vec<String> = raw_urls
        .into_iter()
        .filter(|u| !u.is_empty())
        .map(|u| resolve_url(base_url, &u))
        .collect();

    (out, next_urls)
}

fn parse_chapter_list_xpath(
    body: &str,
    base_url: &str,
    rule: &TocRule,
    list_rule: &str,
    ctx: &mut HashMap<String, String>,
) -> (Vec<BookChapter>, Vec<String>) {
    let package = match sxd_document::parser::parse(body) {
        Ok(p) => p,
        Err(_) => return parse_chapter_list_html(body, base_url, rule, list_rule, ctx),
    };
    let document = package.as_document();
    let scope = select_xpath_scope(
        sxd_xpath::nodeset::Node::Root(document.root()),
        rule.init.as_deref(),
    );
    let items = xpath_select_nodes(scope, list_rule);

    let mut seen_urls = std::collections::HashSet::new();
    let mut out: Vec<BookChapter> = Vec::with_capacity(items.len());
    for item in items {
        let title = eval_field_xpath_with_ctx(
            rule.chapter_name.as_deref().unwrap_or(""),
            item,
            base_url,
            ctx,
        )
        .unwrap_or_default();
        let url = eval_field_xpath_with_ctx(
            rule.chapter_url.as_deref().unwrap_or(""),
            item,
            base_url,
            ctx,
        )
        .unwrap_or_default();
        let tag = eval_field_xpath_with_ctx(
            rule.update_time.as_deref().unwrap_or(""),
            item,
            base_url,
            ctx,
        );
        let is_volume =
            eval_field_xpath_with_ctx(rule.is_volume.as_deref().unwrap_or(""), item, base_url, ctx)
                .map(is_truthy)
                .unwrap_or(false);
        let is_vip =
            eval_field_xpath_with_ctx(rule.is_vip.as_deref().unwrap_or(""), item, base_url, ctx)
                .map(is_truthy)
                .unwrap_or(false);
        let is_pay =
            eval_field_xpath_with_ctx(rule.is_pay.as_deref().unwrap_or(""), item, base_url, ctx)
                .map(is_truthy)
                .unwrap_or(false);
        let url_abs = finalize_chapter_url(base_url, &url, &title, is_volume, out.len());
        if seen_urls.contains(&url_abs) {
            out.retain(|chapter| chapter.url != url_abs);
        } else {
            seen_urls.insert(url_abs.clone());
        }
        out.push(BookChapter {
            title,
            url: url_abs,
            index: out.len() as i32,
            tag,
            is_vip,
            is_pay,
            is_volume,
            ..Default::default()
        });
    }

    let next_urls = rule
        .next_toc_url
        .as_deref()
        .map(|xpath| xpath_eval_strings(scope, xpath))
        .unwrap_or_default()
        .into_iter()
        .filter(|u| !u.is_empty())
        .map(|u| resolve_url(base_url, &u))
        .collect();

    (out, next_urls)
}

fn parse_chapter_list_json(
    body: &str,
    base_url: &str,
    rule: &TocRule,
    list_rule: &str,
    ctx: &mut HashMap<String, String>,
) -> (Vec<BookChapter>, Vec<String>) {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return (vec![], vec![]),
    };
    let scope = select_json_scope(&v, rule.init.as_deref(), base_url, ctx);
    let items = jsonpath::jsonpath_query(&scope, list_rule);

    let mut seen_urls = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let title = eval_field_json_with_ctx(
            rule.chapter_name.as_deref().unwrap_or(""),
            &item,
            base_url,
            ctx,
        )
        .unwrap_or_default();
        let url = eval_field_json_with_ctx(
            rule.chapter_url.as_deref().unwrap_or(""),
            &item,
            base_url,
            ctx,
        )
        .unwrap_or_default();
        let tag = eval_field_json_with_ctx(
            rule.update_time.as_deref().unwrap_or(""),
            &item,
            base_url,
            ctx,
        );
        let is_volume = eval_field_json_with_ctx(
            rule.is_volume.as_deref().unwrap_or(""),
            &item,
            base_url,
            ctx,
        )
        .map(is_truthy)
        .unwrap_or(false);
        let is_vip =
            eval_field_json_with_ctx(rule.is_vip.as_deref().unwrap_or(""), &item, base_url, ctx)
                .map(is_truthy)
                .unwrap_or(false);
        let is_pay =
            eval_field_json_with_ctx(rule.is_pay.as_deref().unwrap_or(""), &item, base_url, ctx)
                .map(is_truthy)
                .unwrap_or(false);
        let url_abs = finalize_chapter_url(base_url, &url, &title, is_volume, out.len());

        if seen_urls.contains(&url_abs) {
            continue;
        }
        seen_urls.insert(url_abs.clone());

        out.push(BookChapter {
            title,
            url: url_abs,
            index: out.len() as i32,
            tag,
            is_vip,
            is_pay,
            is_volume,
            ..Default::default()
        });
    }

    let next_urls: Vec<String> = rule
        .next_toc_url
        .as_ref()
        .map(|r| jsonpath::jsonpath_query(&scope, r))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .filter(|u| !u.is_empty())
        .map(|u| resolve_url(base_url, &u))
        .collect();

    (out, next_urls)
}

fn select_json_scope(
    v: &Value,
    init_rule: Option<&str>,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Value {
    let Some(init_rule) = init_rule.map(str::trim).filter(|s| !s.is_empty()) else {
        return v.clone();
    };

    if let Some(res) = try_put_get_json(init_rule, v, base_url, ctx) {
        let _ = res;
        return v.clone();
    }

    let interpolated = interpolate_json_templates(init_rule, v, base_url, ctx);
    let (pure_rule, _) = split_legado_regex(&interpolated);
    let (pure, _) = extract_js(&pure_rule);
    if pure.is_empty() {
        return v.clone();
    }

    jsonpath::jsonpath_query(v, pure)
        .into_iter()
        .next()
        .unwrap_or_else(|| v.clone())
}

fn pick_json_field(v: &Value, rule: Option<&str>) -> Option<String> {
    let rule = rule?;
    if rule.trim_start().starts_with('$') {
        return jsonpath::jsonpath_first_string(v, rule);
    }
    if let Some(obj) = v.as_object() {
        if let Some(val) = obj.get(rule) {
            return jsonpath::value_to_string(val);
        }
    }
    None
}

fn resolve_url(base: &str, url: &str) -> String {
    let base = normalize_source_url(base);
    let url = normalize_source_url(strip_url_config(url));

    if url.is_empty() {
        return base.to_string();
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    if url.starts_with("//") {
        return format!("https:{}", url);
    }

    let mut base_url = match url::Url::parse(&base) {
        Ok(u) => u,
        Err(_) => return url.to_string(),
    };
    base_url.set_fragment(None);

    match base_url.join(&url) {
        Ok(u) => u.to_string(),
        Err(_) => {
            let base = base.trim_end_matches('/');
            format!("{}/{}", base, url.trim_start_matches('/'))
        }
    }
}

fn interpolate_json_templates(
    rule: &str,
    v: &Value,
    base_url: &str,
    ctx: &HashMap<String, String>,
) -> String {
    let re = regex::Regex::new(r"\{\{(.*?)\}\}").unwrap();
    re.replace_all(rule, |caps: &regex::Captures| {
        let expr = caps.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
        if expr.is_empty() {
            return String::new();
        }

        if let Some(key) = expr
            .strip_prefix("@get:{")
            .and_then(|s| s.strip_suffix('}'))
        {
            return ctx.get(key.trim()).cloned().unwrap_or_default();
        }

        if expr.starts_with('$') {
            return pick_json_field(v, Some(expr)).unwrap_or_default();
        }

        if let Some(val) = ctx.get(expr) {
            return val.clone();
        }

        if let Some(val) = pick_json_field(v, Some(expr)) {
            return val;
        }

        match eval_js(
            expr,
            &serde_json::to_string(v).unwrap_or_default(),
            base_url,
        ) {
            Ok(res) => res,
            Err(_) => String::new(),
        }
    })
    .into_owned()
}

fn interpolate_common_templates(
    rule: &str,
    input: &str,
    base_url: &str,
    ctx: &HashMap<String, String>,
) -> String {
    let get_re = regex::Regex::new(r"@get:\{([^}]+)\}").unwrap();
    let with_get = get_re.replace_all(rule, |caps: &regex::Captures| {
        let key = caps.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
        ctx.get(key).cloned().unwrap_or_default()
    });

    let js_re = regex::Regex::new(r"\{\{(.*?)\}\}").unwrap();
    js_re
        .replace_all(&with_get, |caps: &regex::Captures| {
            let expr = caps.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
            if expr.is_empty() {
                return String::new();
            }
            if let Some(val) = ctx.get(expr) {
                return val.clone();
            }
            eval_js(expr, input, base_url).unwrap_or_default()
        })
        .into_owned()
}

fn strip_url_config(url: &str) -> &str {
    if let Some(idx) = url.find("##$##") {
        &url[..idx]
    } else if let Some(idx) = url.find(",{'webView'") {
        &url[..idx]
    } else if let Some(idx) = url.find(",{\"webView\"") {
        &url[..idx]
    } else {
        url
    }
}

fn extract_js(rule: &str) -> (&str, Option<&str>) {
    if let Some(idx) = rule.find("<js>") {
        if let Some(end_idx) = rule.rfind("</js>") {
            if end_idx > idx {
                let pure = rule[..idx].trim();
                let js = &rule[idx + 4..end_idx];
                return (pure, Some(js));
            }
        }
    }
    if let Some(idx) = rule.find("@js:") {
        let pure = rule[..idx].trim();
        let js = &rule[idx + 4..];
        return (pure, Some(js));
    }
    (rule, None)
}

fn eval_field_html(rule: &str, el: &scraper::ElementRef, base_url: &str) -> Option<String> {
    eval_field_html_with_ctx(rule, el, base_url, &mut HashMap::new())
}

fn eval_field_html_with_ctx(
    rule: &str,
    el: &scraper::ElementRef,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    // Handle mode forcing prefixes
    let rule = rule.trim();
    if rule.starts_with("@css:") {
        let pure = &rule[5..];
        return eval_field_html_with_ctx(pure, el, base_url, ctx);
    }
    if rule.starts_with("@xpath:") {
        // XPath from element - not directly supported, return None
        return None;
    }
    if rule.starts_with("@json:") {
        // JSON from element - not applicable
        return None;
    }

    // Handle @put/@get
    if let Some(res) = try_put_get_html(rule, el, base_url, ctx) {
        return Some(res);
    }

    let input = html::extract_text(el, "textNodes").unwrap_or_default();
    let interpolated_rule = interpolate_common_templates(rule, &input, base_url, ctx);
    let had_templates = interpolated_rule != rule;
    let (pure_rule, regex_part) = split_legado_regex(&interpolated_rule);
    let (pure, js) = extract_js(&pure_rule);

    let mut text = if pure.is_empty() {
        "".to_string()
    } else {
        html::select_text_from_element(el, pure).unwrap_or_default()
    };
    if text.is_empty() && had_templates && !pure.is_empty() {
        text = pure.to_string();
    }

    if let Some(script) = js {
        if let Ok(res) = eval_js(script, &text, base_url) {
            text = res;
        }
    }

    if let Some(reg) = regex_part {
        text = apply_legado_regex(&text, reg);
    }

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn eval_field_html_doc_with_ctx(
    rule: &str,
    doc: &scraper::Html,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    // Handle mode forcing prefixes
    let rule = rule.trim();
    if rule.starts_with("@css:") {
        let pure = &rule[5..];
        return eval_field_html_doc_with_ctx(pure, doc, base_url, ctx);
    }
    if rule.starts_with("@xpath:") {
        let pure = &rule[7..];
        return html::select_xpath(&doc.html(), pure).first().cloned();
    }

    if let Some(res) = try_put_get_html_doc(rule, doc, base_url, ctx) {
        return Some(res);
    }

    let interpolated_rule = interpolate_common_templates(rule, &doc.html(), base_url, ctx);
    let had_templates = interpolated_rule != rule;
    let (pure, js) = extract_js(&interpolated_rule);
    let mut text = if pure.is_empty() {
        "".to_string()
    } else {
        html::select_text(doc, pure).unwrap_or_default()
    };
    if text.is_empty() && had_templates && !pure.is_empty() {
        text = pure.to_string();
    }

    if let Some(script) = js {
        if let Ok(res) = eval_js(script, &text, base_url) {
            return Some(res);
        }
        return Some(text);
    }

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn eval_field_json(rule: &str, v: &Value, base_url: &str) -> Option<String> {
    eval_field_json_with_ctx(rule, v, base_url, &mut HashMap::new())
}

fn eval_field_xpath(
    rule: &str,
    node: sxd_xpath::nodeset::Node<'_>,
    base_url: &str,
) -> Option<String> {
    eval_field_xpath_with_ctx(rule, node, base_url, &mut HashMap::new())
}

fn eval_field_xpath_with_ctx(
    rule: &str,
    node: sxd_xpath::nodeset::Node<'_>,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    if rule.trim().is_empty() {
        return None;
    }
    if let Some(res) = try_put_get_xpath(rule, node, base_url, ctx) {
        return Some(res);
    }

    let interpolated_rule = interpolate_common_templates(rule, &node.string_value(), base_url, ctx);
    let had_templates = interpolated_rule != rule;
    let (pure_rule, regex_part) = split_legado_regex(&interpolated_rule);
    let (pure, js) = extract_js(&pure_rule);
    let mut text = if pure.trim().is_empty() {
        node.string_value()
    } else {
        xpath_eval_strings(node, pure)
            .into_iter()
            .next()
            .unwrap_or_default()
    };
    if text.is_empty() && had_templates && !pure.is_empty() {
        text = pure.to_string();
    }

    if let Some(script) = js {
        if let Ok(res) = eval_js(script, &text, base_url) {
            text = res;
        }
    }
    if let Some(reg) = regex_part {
        text = apply_legado_regex(&text, reg);
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn select_xpath_scope<'a>(
    node: sxd_xpath::nodeset::Node<'a>,
    init_rule: Option<&str>,
) -> sxd_xpath::nodeset::Node<'a> {
    let Some(init_rule) = init_rule.map(str::trim).filter(|s| !s.is_empty()) else {
        return node;
    };
    xpath_select_nodes(node, init_rule)
        .into_iter()
        .next()
        .unwrap_or(node)
}

fn xpath_select_nodes<'a>(
    node: sxd_xpath::nodeset::Node<'a>,
    xpath: &str,
) -> Vec<sxd_xpath::nodeset::Node<'a>> {
    let xpath = xpath.trim();
    if xpath.is_empty() {
        return vec![];
    }
    let context = XPathContext::new();
    match XPathFactory::new().build(xpath) {
        Ok(Some(expr)) => match expr.evaluate(&context, node) {
            Ok(XPathValue::Nodeset(ns)) => ns.document_order(),
            _ => vec![],
        },
        _ => vec![],
    }
}

fn xpath_eval_strings(node: sxd_xpath::nodeset::Node<'_>, xpath: &str) -> Vec<String> {
    let xpath = xpath.trim();
    if xpath.is_empty() {
        return vec![];
    }
    let context = XPathContext::new();
    match XPathFactory::new().build(xpath) {
        Ok(Some(expr)) => match expr.evaluate(&context, node) {
            Ok(XPathValue::Nodeset(ns)) => ns
                .document_order()
                .into_iter()
                .map(|n| n.string_value())
                .collect(),
            Ok(XPathValue::String(s)) => vec![s],
            Ok(XPathValue::Number(n)) => vec![n.to_string()],
            Ok(XPathValue::Boolean(b)) => vec![b.to_string()],
            Err(_) => vec![],
        },
        _ => vec![],
    }
}

fn eval_field_json_with_ctx(
    rule: &str,
    v: &Value,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    if let Some(res) = try_put_get_json(rule, v, base_url, ctx) {
        return Some(res);
    }

    let interpolated_rule = interpolate_json_templates(rule, v, base_url, ctx);
    let (pure_rule, regex_part) = split_legado_regex(&interpolated_rule);
    let (pure, js) = extract_js(&pure_rule);

    let mut text = if pure.is_empty() {
        "".to_string()
    } else if pure.contains("{{") && pure.contains("}}") {
        pure.to_string()
    } else if pure.contains('/')
        || pure.contains('?')
        || pure.contains('&')
        || pure.contains('=')
        || pure.contains(',')
    {
        pure.to_string()
    } else {
        pick_json_field(v, Some(pure)).unwrap_or_default()
    };

    if let Some(script) = js {
        if let Ok(res) = eval_js(script, &text, base_url) {
            text = res;
        }
    }

    if let Some(reg) = regex_part {
        text = apply_legado_regex(&text, reg);
    }

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn try_put_get_html(
    rule: &str,
    el: &scraper::ElementRef,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    if rule.starts_with("@put:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let inner = &content[1..content.len() - 1];
            for part in inner.split(',') {
                if let Some(idx) = part.find(':') {
                    let key = part[..idx].trim();
                    let val_rule = part[idx + 1..].trim().trim_matches('"');
                    let val =
                        eval_field_html_with_ctx(val_rule, el, base_url, ctx).unwrap_or_default();
                    ctx.insert(key.to_string(), val);
                }
            }
        }
        return Some("".to_string());
    }
    if rule.starts_with("@get:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let key = &content[1..content.len() - 1].trim();
            return ctx.get(*key).cloned();
        }
    }
    None
}

fn try_put_get_html_doc(
    rule: &str,
    doc: &scraper::Html,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    if rule.starts_with("@put:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let inner = &content[1..content.len() - 1];
            for part in inner.split(',') {
                if let Some(idx) = part.find(':') {
                    let key = part[..idx].trim();
                    let val_rule = part[idx + 1..].trim().trim_matches('"');
                    let val = eval_field_html_doc_with_ctx(val_rule, doc, base_url, ctx)
                        .unwrap_or_default();
                    ctx.insert(key.to_string(), val);
                }
            }
        }
        return Some("".to_string());
    }
    if rule.starts_with("@get:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let key = &content[1..content.len() - 1].trim();
            return ctx.get(*key).cloned();
        }
    }
    None
}

fn try_put_get_json(
    rule: &str,
    v: &Value,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    if rule.starts_with("@put:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let inner = &content[1..content.len() - 1];
            for part in inner.split(',') {
                if let Some(idx) = part.find(':') {
                    let key = part[..idx].trim();
                    let val_rule = part[idx + 1..].trim().trim_matches('"');
                    let val =
                        eval_field_json_with_ctx(val_rule, v, base_url, ctx).unwrap_or_default();
                    ctx.insert(key.to_string(), val);
                }
            }
        }
        return Some("".to_string());
    }
    if rule.starts_with("@get:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let key = &content[1..content.len() - 1].trim();
            return ctx.get(*key).cloned();
        }
    }
    None
}

fn try_put_get_xpath(
    rule: &str,
    node: sxd_xpath::nodeset::Node<'_>,
    base_url: &str,
    ctx: &mut HashMap<String, String>,
) -> Option<String> {
    if rule.starts_with("@put:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let inner = &content[1..content.len() - 1];
            for part in inner.split(',') {
                if let Some(idx) = part.find(':') {
                    let key = part[..idx].trim();
                    let val_rule = part[idx + 1..].trim().trim_matches('"');
                    let val = eval_field_xpath_with_ctx(val_rule, node, base_url, ctx)
                        .unwrap_or_default();
                    ctx.insert(key.to_string(), val);
                }
            }
        }
        return Some(String::new());
    }
    if rule.starts_with("@get:") {
        let content = &rule[5..];
        if content.starts_with('{') && content.ends_with('}') {
            let key = &content[1..content.len() - 1].trim();
            return ctx.get(*key).cloned();
        }
    }
    None
}

fn split_legado_regex(rule: &str) -> (String, Option<&str>) {
    if let Some(idx) = rule.find("##") {
        let (pure, reg) = rule.split_at(idx);
        return (pure.trim().to_string(), Some(reg));
    }
    (rule.to_string(), None)
}

fn apply_legado_regex(text: &str, regex_part: &str) -> String {
    if regex_part.trim().is_empty() {
        return text.to_string();
    }

    // Handle ### suffix for first-match-only replacement
    let (regex_part, first_only) = if regex_part.ends_with("###") {
        (&regex_part[..regex_part.len() - 3], true)
    } else {
        (regex_part, false)
    };

    let parts: Vec<&str> = regex_part.split("##").collect();

    // Support: ##regex##replace
    let start_idx = if regex_part.starts_with("##") { 1 } else { 0 };

    let mut out = text.to_string();
    let mut i = start_idx;
    while i + 1 < parts.len() {
        let regex = parts[i];
        if regex.is_empty() {
            i += 1;
            continue;
        }

        let replace = parts[i + 1];

        if first_only && i + 2 >= parts.len() {
            // Last replacement with ### suffix - first match only
            out = apply_regex_replace_first(&out, regex, replace);
        } else {
            out = apply_regex_replace(&out, regex, replace);
        }
        i += 2;
    }
    out
}

/// Apply regex replacement to first match only
fn apply_regex_replace_first(text: &str, pattern: &str, replacement: &str) -> String {
    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };
    re.replace(text, replacement).to_string()
}

fn normalize_list_rule(rule: &str) -> (&str, bool) {
    let rule = rule.trim();
    if let Some(rest) = rule.strip_prefix('-') {
        return (rest.trim(), true);
    }
    if let Some(rest) = rule.strip_prefix('+') {
        return (rest.trim(), false);
    }
    (rule, false)
}

fn strip_mode_prefix(rule: &str) -> &str {
    let rule = rule.trim();
    if let Some(rest) = rule
        .strip_prefix("<js>")
        .and_then(|s| s.strip_suffix("</js>"))
    {
        return rest;
    }
    for prefix in [
        "@css:", "@CSS:", "@xpath:", "@XPath:", "@XPATH:", "@json:", "@Json:", "@JSON:", "@regex:",
        "@Regex:", "@js:", "js:",
    ] {
        if let Some(rest) = rule.strip_prefix(prefix) {
            return rest;
        }
    }
    rule
}

fn strip_js_rule(rule: &str) -> &str {
    let rule = rule.trim();
    if let Some(rest) = rule
        .strip_prefix("<js>")
        .and_then(|s| s.strip_suffix("</js>"))
    {
        return rest;
    }
    if let Some(rest) = rule.strip_prefix("@js:") {
        return rest;
    }
    if let Some(rest) = rule.strip_prefix("js:") {
        return rest;
    }
    rule
}

fn prepare_toc_body(body: &str, base_url: &str, rule: &TocRule) -> String {
    let Some(script) = rule
        .pre_update_js
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    else {
        return body.to_string();
    };
    match eval_js(strip_js_rule(script), body, base_url) {
        Ok(result) if !result.trim().is_empty() => result,
        _ => body.to_string(),
    }
}

fn apply_toc_format_js(chapters: &mut [BookChapter], format_js: Option<&str>, base_url: &str) {
    let Some(script) = format_js.filter(|s| !s.trim().is_empty()) else {
        return;
    };
    let script = strip_js_rule(script);
    for (index, chapter) in chapters.iter_mut().enumerate() {
        let mut bindings = HashMap::new();
        bindings.insert("index".to_string(), json!(index + 1));
        bindings.insert("title".to_string(), json!(chapter.title.clone()));
        bindings.insert(
            "chapter".to_string(),
            serde_json::to_value(&*chapter).unwrap_or_else(|_| json!({})),
        );
        if let Ok(result) = eval_js_with_bindings(script, &chapter.title, base_url, &bindings) {
            if !result.trim().is_empty() {
                chapter.title = result;
            }
        }
    }
}

fn parse_js_output_items(output: &str) -> Option<Vec<Value>> {
    let value = serde_json::from_str::<Value>(output.trim()).ok()?;
    match value {
        Value::Array(items) => Some(items),
        Value::Object(_) => Some(vec![value]),
        _ => None,
    }
}

fn search_book_from_book(book: Book) -> Option<SearchBook> {
    if book.name.trim().is_empty() {
        return None;
    }
    Some(SearchBook {
        name: book.name,
        author: book.author,
        book_url: book.book_url,
        origin: book.origin,
        cover_url: book.cover_url,
        intro: book.intro,
        kind: book.kind,
        last_chapter: book.latest_chapter_title,
        update_time: book.update_time,
        word_count: book.word_count,
        book_source_urls: None,
    })
}

fn build_search_book_from_json(
    source: &BookSource,
    item: &Value,
    base_url: &str,
    rule: &SearchRule,
) -> Option<SearchBook> {
    let mut ctx = HashMap::new();
    let name =
        eval_field_json_with_ctx(rule.name.as_deref().unwrap_or(""), item, base_url, &mut ctx)
            .unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    let author = eval_field_json_with_ctx(
        rule.author.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    )
    .unwrap_or_default();
    let book_url = eval_field_json_with_ctx(
        rule.book_url.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    )
    .unwrap_or_default();
    let cover_url = eval_field_json_with_ctx(
        rule.cover_url.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    )
    .map(|u| resolve_url(base_url, &u));
    let intro = eval_field_json_with_ctx(
        rule.intro.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    );
    let kind =
        eval_field_json_with_ctx(rule.kind.as_deref().unwrap_or(""), item, base_url, &mut ctx);
    let last_chapter = eval_field_json_with_ctx(
        rule.last_chapter.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    );
    let update_time = eval_field_json_with_ctx(
        rule.update_time.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    );
    let word_count = eval_field_json_with_ctx(
        rule.word_count.as_deref().unwrap_or(""),
        item,
        base_url,
        &mut ctx,
    );
    Some(SearchBook {
        name,
        author,
        book_url: resolve_url(base_url, &book_url),
        origin: source.book_source_url.clone(),
        cover_url,
        intro,
        kind,
        last_chapter,
        update_time,
        word_count,
        book_source_urls: None,
    })
}

fn build_chapter_from_json(
    item: &Value,
    base_url: &str,
    rule: &TocRule,
    ctx: &mut HashMap<String, String>,
    index: usize,
) -> Option<BookChapter> {
    let title = eval_field_json_with_ctx(
        rule.chapter_name.as_deref().unwrap_or(""),
        item,
        base_url,
        ctx,
    )
    .unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    let raw_url = eval_field_json_with_ctx(
        rule.chapter_url.as_deref().unwrap_or(""),
        item,
        base_url,
        ctx,
    )
    .unwrap_or_default();
    let tag = eval_field_json_with_ctx(
        rule.update_time.as_deref().unwrap_or(""),
        item,
        base_url,
        ctx,
    );
    let is_volume =
        eval_field_json_with_ctx(rule.is_volume.as_deref().unwrap_or(""), item, base_url, ctx)
            .map(is_truthy)
            .unwrap_or(false);
    let is_vip =
        eval_field_json_with_ctx(rule.is_vip.as_deref().unwrap_or(""), item, base_url, ctx)
            .map(is_truthy)
            .unwrap_or(false);
    let is_pay =
        eval_field_json_with_ctx(rule.is_pay.as_deref().unwrap_or(""), item, base_url, ctx)
            .map(is_truthy)
            .unwrap_or(false);
    Some(BookChapter {
        title: title.clone(),
        url: finalize_chapter_url(base_url, &raw_url, &title, is_volume, index),
        index: index as i32,
        tag,
        is_vip,
        is_pay,
        is_volume,
    })
}

fn capture_rule_value(rule: Option<&str>, captures: &regex::Captures<'_>) -> Option<String> {
    let rule = rule?.trim();
    if rule.is_empty() {
        return None;
    }
    let placeholder = regex::Regex::new(r"\$(\d{1,2})").unwrap();
    let replaced = placeholder.replace_all(rule, |cap: &regex::Captures| {
        let index = cap
            .get(1)
            .and_then(|m| m.as_str().parse::<usize>().ok())
            .unwrap_or(0);
        if index == 0 {
            return cap
                .get(0)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
        }
        captures
            .get(index)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| {
                cap.get(0)
                    .map(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
    });
    let (pure, regex_part) = split_legado_regex(&replaced);
    let mut output = pure;
    if let Some(regex_part) = regex_part {
        output = apply_legado_regex(&output, regex_part);
    }
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn finalize_chapter_url(
    base_url: &str,
    raw_url: &str,
    title: &str,
    is_volume: bool,
    index: usize,
) -> String {
    if !raw_url.trim().is_empty() {
        return resolve_url(base_url, raw_url);
    }
    if is_volume {
        return format!("{}{}", title, index);
    }
    base_url.to_string()
}

fn is_truthy(value: String) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "0" | "false" | "null" | "none" | "no" | "off"
    )
}

/// Extract chapter number from title
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::book_source::BookSource;
    use crate::model::rule::{BookInfoRule, SearchRule, TocRule};

    #[test]
    fn test_detect_mode() {
        let engine = RuleEngine::new().unwrap();

        assert_eq!(engine.detect_mode("@css:.test", ""), ParseMode::Css);
        assert_eq!(engine.detect_mode("@xpath://div", ""), ParseMode::XPath);
        assert_eq!(engine.detect_mode("$.data.list", ""), ParseMode::JsonPath);
        assert_eq!(engine.detect_mode("/html/body/div", ""), ParseMode::XPath);
        assert_eq!(engine.detect_mode(".class", ""), ParseMode::Css);
        assert_eq!(engine.detect_mode("js:return 1", ""), ParseMode::Js);
        assert_eq!(engine.detect_mode("<js>return 1</js>", ""), ParseMode::Js);
    }

    #[test]
    fn test_apply_legado_regex() {
        let text = "Hello World 123 456";

        // Test basic replacement (all matches)
        let result = apply_legado_regex(text, "##\\d+##NUM");
        assert_eq!(result, "Hello World NUM NUM");

        // Test first match only (###)
        let result = apply_legado_regex(text, "##\\d+##NUM###");
        assert_eq!(result, "Hello World NUM 456");
    }

    #[test]
    fn test_search_detail_fallback_uses_book_info_rules() {
        let engine = RuleEngine::new().unwrap();
        let source = BookSource {
            book_source_name: "Test".to_string(),
            book_source_url: "https://source.example".to_string(),
            rule_search: Some(SearchRule {
                book_list: Some(String::new()),
                ..Default::default()
            }),
            rule_book_info: Some(BookInfoRule {
                name: Some(".name@text".to_string()),
                author: Some(".author@text".to_string()),
                intro: Some(".intro@text".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let body = r#"
            <div class="name">Fallback Book</div>
            <div class="author">Fallback Author</div>
            <div class="intro">Fallback Intro</div>
        "#;

        let results = engine.search_books(&source, body, "https://books.example/detail/1");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Fallback Book");
        assert_eq!(results[0].author, "Fallback Author");
        assert_eq!(results[0].intro.as_deref(), Some("Fallback Intro"));
    }

    #[test]
    fn test_search_books_regex_list() {
        let engine = RuleEngine::new().unwrap();
        let source = BookSource {
            book_source_name: "Regex".to_string(),
            book_source_url: "https://source.example".to_string(),
            rule_search: Some(SearchRule {
                book_list: Some(r#":<a href="([^"]+)">([^<]+)</a>"#.to_string()),
                name: Some("$2".to_string()),
                book_url: Some("$1".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let body = r#"<a href="/book/1">One</a><a href="/book/2">Two</a>"#;

        let results = engine.search_books(&source, body, "https://books.example");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "One");
        assert_eq!(results[0].book_url, "https://books.example/book/1");
        assert_eq!(results[1].name, "Two");
    }

    #[test]
    fn test_search_books_js_json_list() {
        let engine = RuleEngine::new().unwrap();
        let source = BookSource {
            book_source_name: "JS".to_string(),
            book_source_url: "https://source.example".to_string(),
            rule_search: Some(SearchRule {
                book_list: Some(
                    "js:JSON.stringify([{name:'Alpha',author:'Tester',bookUrl:'/alpha'}])"
                        .to_string(),
                ),
                name: Some("name".to_string()),
                author: Some("author".to_string()),
                book_url: Some("bookUrl".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let results = engine.search_books(&source, "<html></html>", "https://books.example");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alpha");
        assert_eq!(results[0].author, "Tester");
        assert_eq!(results[0].book_url, "https://books.example/alpha");
    }

    #[test]
    fn test_chapter_list_js_and_format_js() {
        let engine = RuleEngine::new().unwrap();
        let source = BookSource {
            book_source_name: "JS TOC".to_string(),
            book_source_url: "https://source.example".to_string(),
            rule_toc: Some(TocRule {
                chapter_list: Some("js:JSON.stringify([{chapterName:'One',chapterUrl:'/1',isVip:'1'},{chapterName:'Two',chapterUrl:'/2',isPay:'true'}])".to_string()),
                chapter_name: Some("chapterName".to_string()),
                chapter_url: Some("chapterUrl".to_string()),
                is_vip: Some("isVip".to_string()),
                is_pay: Some("isPay".to_string()),
                format_js: Some("`${index}.${title}`".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (chapters, next_urls) =
            engine.chapter_list(&source, "<html></html>", "https://books.example");
        assert!(next_urls.is_empty());
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "1.One");
        assert_eq!(chapters[0].url, "https://books.example/1");
        assert!(chapters[0].is_vip);
        assert_eq!(chapters[1].title, "2.Two");
        assert!(chapters[1].is_pay);
    }

    #[test]
    fn test_chapter_list_keeps_real_chapterlist_container() {
        let engine = RuleEngine::new().unwrap();
        let source = BookSource {
            book_source_name: "HTML TOC".to_string(),
            book_source_url: "https://source.example".to_string(),
            rule_toc: Some(TocRule {
                chapter_list: Some("#chapterlist a".to_string()),
                chapter_name: Some("@text".to_string()),
                chapter_url: Some("@href".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let body = r#"
            <div id="chapterlist">
                <a href="/1">第一章</a>
                <a href="/2">第二章</a>
            </div>
        "#;

        let (chapters, next_urls) = engine.chapter_list(&source, body, "https://books.example");
        assert!(next_urls.is_empty());
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].url, "https://books.example/1");
        assert_eq!(chapters[1].url, "https://books.example/2");
    }

    #[test]
    fn test_search_books_js_uses_js_lib() {
        let engine = RuleEngine::new().unwrap();
        let source = BookSource {
            book_source_name: "JS Lib".to_string(),
            book_source_url: "https://source.example".to_string(),
            js_lib: Some("function buildName(v){ return v + '-lib'; }".to_string()),
            rule_search: Some(SearchRule {
                book_list: Some("js:JSON.stringify([{name:buildName('Alpha'),author:'Tester',bookUrl:'/alpha'}])".to_string()),
                name: Some("name".to_string()),
                author: Some("author".to_string()),
                book_url: Some("bookUrl".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let results = engine.search_books(&source, "<html></html>", "https://books.example");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alpha-lib");
    }

    #[test]
    fn test_book_info_html_interpolates_get_template() {
        let source = BookSource {
            book_source_name: "Info".to_string(),
            book_source_url: "https://source.example".to_string(),
            rule_book_info: Some(BookInfoRule {
                init: Some("@put:{alias:.name@text}".to_string()),
                name: Some("Book-@get:{alias}".to_string()),
                author: Some(".author@text".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let body = r#"<div class="name">Alias</div><div class="author">Tester</div>"#;
        let mut ctx = HashMap::new();
        let book = parse_book_info_html(
            &source,
            body,
            "https://books.example/detail/1",
            &source.rule_book_info.clone().unwrap(),
            "https://books.example/detail/1",
            &mut ctx,
        );
        assert_eq!(book.name, "Book-Alias");
        assert_eq!(book.author, "Tester");
    }
}

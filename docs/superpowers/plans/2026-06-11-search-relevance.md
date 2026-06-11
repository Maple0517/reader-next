# Search Relevance Ranking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce noisy novel search results by ranking exact/strong title matches first and filtering very weak matches from multi-source search.

**Architecture:** Add a small backend relevance module that scores `SearchBook` against the query after parsing book-source results. Apply it in both JSON multi-search and SSE multi-search before dedupe/output, so all clients get consistent ordering. Keep single-source raw search mostly unchanged except for optional sort only if the caller goes through shared helpers.

**Tech Stack:** Rust 2021, Axum handlers, existing `SearchBook` model, `cargo test`; Vue 3/Vite frontend only needs type/UI wiring if we expose score/debug metadata.

---

## File Structure

- Modify: `/Users/maple/Documents/reader/src/service/mod.rs`
  - Export the new `search_relevance` module.
- Create: `/Users/maple/Documents/reader/src/service/search_relevance.rs`
  - Owns query/result normalization, scoring, weak-match filtering, and stable sorting.
- Modify: `/Users/maple/Documents/reader/src/api/handlers/book.rs`
  - Uses the relevance helper in `search_book_multi`, `merge_search_results`, and `search_book_multi_sse`.
  - Adds focused unit tests in the existing `#[cfg(test)] mod tests` block.
- Optional modify: `/Users/maple/Documents/reader/frontend/src/types/index.ts`
  - Only needed if the backend exposes `searchScore` to the client. Default plan does not expose it.

## Behavior Rules

For query `没钱修什么仙`:

1. Keep and rank highest:
   - exact compact title: `没钱修什么仙`
   - title contains compact query: `我在异界没钱修什么仙`
2. Keep but rank lower:
   - title matches most meaningful query segments in order: `没钱修仙是什么体验`
3. Hide from default output:
   - weak overlap only: `修什么仙造作啊`
   - generic token noise: `什么？我家老祖竟是仙帝？`
4. Never filter everything from a bad source if all results are weak; instead return the best weak results after sorting. This avoids false empty states for unusual book names.
5. De-duplication stays based on normalized `name + author`; when duplicates exist, keep the higher-scored result as the representative.

---

### Task 1: Add backend relevance scoring helper

**Files:**
- Create: `/Users/maple/Documents/reader/src/service/search_relevance.rs`
- Modify: `/Users/maple/Documents/reader/src/service/mod.rs`

- [ ] **Step 1: Write failing tests for scoring and filtering**

Add this file at `/Users/maple/Documents/reader/src/service/search_relevance.rs`:

```rust
use crate::model::search::SearchBook;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchRelevance {
    pub score: i32,
    pub strong_match: bool,
}

pub fn score_search_book(query: &str, book: &SearchBook) -> SearchRelevance {
    let query = normalize_search_text(query);
    let title = normalize_search_text(&book.name);
    let author = normalize_search_text(&book.author);

    if query.is_empty() {
        return SearchRelevance {
            score: 0,
            strong_match: true,
        };
    }

    let mut score = 0;
    let mut strong_match = false;

    if title == query {
        score += 10_000;
        strong_match = true;
    } else if title.contains(&query) {
        score += 8_000;
        strong_match = true;
    } else if query.contains(&title) && title.chars().count() >= 2 {
        score += 3_000;
        strong_match = true;
    }

    let query_chars: Vec<char> = query.chars().collect();
    let title_chars: Vec<char> = title.chars().collect();
    let common_count = query_chars
        .iter()
        .filter(|ch| title_chars.contains(ch))
        .count() as i32;
    let query_len = query_chars.len().max(1) as i32;
    score += common_count * 100;

    if common_count * 100 / query_len >= 60 {
        strong_match = true;
    }

    let ordered_count = ordered_subsequence_match_count(&query_chars, &title_chars) as i32;
    score += ordered_count * 25;

    if author == query {
        score += 1_000;
        strong_match = true;
    }

    SearchRelevance {
        score,
        strong_match,
    }
}

pub fn sort_and_filter_search_results(query: &str, mut books: Vec<SearchBook>) -> Vec<SearchBook> {
    if query.trim().is_empty() || books.len() <= 1 {
        return books;
    }

    books.sort_by(|a, b| {
        let a_score = score_search_book(query, a).score;
        let b_score = score_search_book(query, b).score;
        b_score
            .cmp(&a_score)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.author.cmp(&b.author))
            .then_with(|| a.origin.cmp(&b.origin))
    });

    let strong: Vec<SearchBook> = books
        .iter()
        .filter(|book| score_search_book(query, book).strong_match)
        .cloned()
        .collect();

    if strong.is_empty() {
        books
    } else {
        strong
    }
}

fn normalize_search_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '《' | '》' | '「' | '」' | '“' | '”' | '"' | '\'' | ':' | '：' | '-' | '_' | '，' | ',' | '。' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn ordered_subsequence_match_count(query: &[char], title: &[char]) -> usize {
    let mut title_index = 0;
    let mut count = 0;
    for query_ch in query {
        while title_index < title.len() {
            let title_ch = title[title_index];
            title_index += 1;
            if *query_ch == title_ch {
                count += 1;
                break;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::{score_search_book, sort_and_filter_search_results};
    use crate::model::search::SearchBook;

    fn book(name: &str) -> SearchBook {
        SearchBook {
            name: name.to_string(),
            author: "作者".to_string(),
            origin: "https://source.test".to_string(),
            book_url: format!("https://source.test/{}", name),
            ..SearchBook::default()
        }
    }

    #[test]
    fn exact_title_scores_above_partial_noise() {
        let exact = score_search_book("没钱修什么仙", &book("没钱修什么仙"));
        let partial = score_search_book("没钱修什么仙", &book("修什么仙造作啊"));

        assert!(exact.strong_match);
        assert!(exact.score > partial.score);
    }

    #[test]
    fn full_query_containment_is_strong() {
        let score = score_search_book("没钱修什么仙", &book("我在异界没钱修什么仙"));

        assert!(score.strong_match);
        assert!(score.score >= 8_000);
    }

    #[test]
    fn weak_token_overlap_is_filtered_when_strong_results_exist() {
        let results = sort_and_filter_search_results(
            "没钱修什么仙",
            vec![
                book("修什么仙造作啊"),
                book("什么？我家老祖竟是仙帝？"),
                book("没钱修什么仙"),
                book("我在异界没钱修什么仙"),
            ],
        );

        let names: Vec<String> = results.into_iter().map(|book| book.name).collect();
        assert_eq!(names, vec!["没钱修什么仙", "我在异界没钱修什么仙"]);
    }

    #[test]
    fn weak_results_are_not_all_dropped_when_no_strong_match_exists() {
        let results = sort_and_filter_search_results(
            "不存在的冷门书名",
            vec![book("普通玄幻"), book("冷门修仙")],
        );

        assert_eq!(results.len(), 2);
    }
}
```

- [ ] **Step 2: Export the module**

Modify `/Users/maple/Documents/reader/src/service/mod.rs`:

```rust
pub mod ai_book_service;
pub mod ai_model_service;
pub mod book_group_service;
pub mod book_service;
pub mod book_source_service;
pub mod json_document_service;
pub mod local_txt_book;
pub mod search_relevance;
pub mod update_service;
pub mod user_service;
```

- [ ] **Step 3: Run focused tests and verify they pass**

Run:

```bash
cd /Users/maple/Documents/reader
cargo test search_relevance
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Commit Task 1**

```bash
cd /Users/maple/Documents/reader
git add src/service/mod.rs src/service/search_relevance.rs
git commit -m "feat: add search relevance scoring"
```

---

### Task 2: Apply relevance sorting/filtering to normal multi-search

**Files:**
- Modify: `/Users/maple/Documents/reader/src/api/handlers/book.rs`

- [ ] **Step 1: Add import**

Near the existing imports in `/Users/maple/Documents/reader/src/api/handlers/book.rs`, add:

```rust
use crate::service::search_relevance::{score_search_book, sort_and_filter_search_results};
```

- [ ] **Step 2: Update `search_book_multi` to pass the query into merge**

Replace this logic:

```rust
let merged = merge_search_results(results);
```

with:

```rust
let merged = merge_search_results(&key, results);
```

- [ ] **Step 3: Change `merge_search_results` signature and representative selection**

Replace the function header:

```rust
fn merge_search_results(
    results: Vec<crate::model::search::SearchBook>,
) -> Vec<crate::model::search::SearchBook> {
```

with:

```rust
fn merge_search_results(
    query: &str,
    results: Vec<crate::model::search::SearchBook>,
) -> Vec<crate::model::search::SearchBook> {
```

Inside the `if let Some(existing) = merged.get_mut(&key)` block, before filling missing fields, add:

```rust
let incoming_score = score_search_book(query, &book).score;
let existing_score = score_search_book(query, existing).score;
if incoming_score > existing_score {
    let existing_origin = existing.origin.clone();
    let existing_urls = existing.book_source_urls.clone();
    let mut replacement = book.clone();
    replacement.book_source_urls = existing_urls.or_else(|| Some(vec![existing_origin]));
    *existing = replacement;
}
```

At the end of `merge_search_results`, replace:

```rust
let mut result: Vec<SearchBook> = merged.into_values().collect();
// Sort by name for consistent ordering
result.sort_by(|a, b| a.name.cmp(&b.name));
result
```

with:

```rust
let result: Vec<SearchBook> = merged.into_values().collect();
sort_and_filter_search_results(query, result)
```

- [ ] **Step 4: Add unit tests to existing `#[cfg(test)] mod tests`**

Extend the `use super::{...};` list in `/Users/maple/Documents/reader/src/api/handlers/book.rs` tests to include `merge_search_results`.

Add these tests inside the existing test module:

```rust
#[test]
fn merge_search_results_ranks_exact_matches_first_and_filters_noise() {
    let books = vec![
        SearchBook {
            name: "修什么仙造作啊".to_string(),
            author: "雏禾".to_string(),
            origin: "source-a".to_string(),
            book_url: "a".to_string(),
            ..SearchBook::default()
        },
        SearchBook {
            name: "没钱修什么仙".to_string(),
            author: "封七月".to_string(),
            origin: "source-b".to_string(),
            book_url: "b".to_string(),
            ..SearchBook::default()
        },
        SearchBook {
            name: "我在异界没钱修什么仙".to_string(),
            author: "一只鱼".to_string(),
            origin: "source-c".to_string(),
            book_url: "c".to_string(),
            ..SearchBook::default()
        },
    ];

    let merged = merge_search_results("没钱修什么仙", books);
    let names: Vec<String> = merged.into_iter().map(|book| book.name).collect();

    assert_eq!(names, vec!["没钱修什么仙", "我在异界没钱修什么仙"]);
}

#[test]
fn merge_search_results_keeps_best_duplicate_representative() {
    let books = vec![
        SearchBook {
            name: "没钱修什么仙".to_string(),
            author: "封七月".to_string(),
            origin: "source-a".to_string(),
            book_url: "weak-url".to_string(),
            cover_url: None,
            ..SearchBook::default()
        },
        SearchBook {
            name: "没钱修什么仙".to_string(),
            author: "封七月".to_string(),
            origin: "source-b".to_string(),
            book_url: "strong-url".to_string(),
            cover_url: Some("cover.jpg".to_string()),
            ..SearchBook::default()
        },
    ];

    let merged = merge_search_results("没钱修什么仙", books);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].name, "没钱修什么仙");
    assert!(merged[0].book_source_urls.as_ref().is_some_and(|urls| urls.contains(&"source-a".to_string()) && urls.contains(&"source-b".to_string())));
}
```

- [ ] **Step 5: Run focused tests**

```bash
cd /Users/maple/Documents/reader
cargo test merge_search_results
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit Task 2**

```bash
cd /Users/maple/Documents/reader
git add src/api/handlers/book.rs
git commit -m "feat: rank multi-source search results"
```

---

### Task 3: Apply relevance to SSE search batches

**Files:**
- Modify: `/Users/maple/Documents/reader/src/api/handlers/book.rs`

- [ ] **Step 1: Sort/filter each source batch before SSE output**

In `search_book_multi_sse`, replace:

```rust
let mut batch = Vec::new();
for b in list {
    let key = format!("{}_{}", b.name, b.author);
    if !result_map.contains(&key) {
        result_map.insert(key);
        batch.push(b);
    }
}
```

with:

```rust
let ranked_list = sort_and_filter_search_results(&key, list);
let mut batch = Vec::new();
for b in ranked_list {
    let result_key = b.merge_key();
    if result_map.insert(result_key) {
        batch.push(b);
    }
}
```

- [ ] **Step 2: Avoid stopping on filtered weak noise too early**

Keep this existing limit logic unchanged:

```rust
if total >= search_size {
    stop_adding = true;
}
```

Reason: after batch-level filtering, `total` now counts relevant results, not raw noisy results.

- [ ] **Step 3: Add helper-level regression test instead of async SSE integration**

If Task 2 tests already cover sorting/filtering and Task 1 covers helper behavior, no additional SSE integration test is needed. The SSE change uses the same pure helper and existing result-map behavior.

Run:

```bash
cd /Users/maple/Documents/reader
cargo test search_relevance merge_search_results
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Commit Task 3**

```bash
cd /Users/maple/Documents/reader
git add src/api/handlers/book.rs
git commit -m "feat: filter noisy sse search batches"
```

---

### Task 4: Verify frontend behavior without adding frontend complexity

**Files:**
- Read-only verification: `/Users/maple/Documents/reader/frontend/src/components/SearchResults.vue`
- Read-only verification: `/Users/maple/Documents/reader/frontend/src/api/search.ts`

- [ ] **Step 1: Confirm frontend does not need a new field**

Check that `SearchResults.vue` renders `displayResults` from backend order:

```bash
cd /Users/maple/Documents/reader
rg -n "displayResults|searchResults = \[\.\.\.shelfStore.searchResults" frontend/src/components/SearchResults.vue
```

Expected relevant lines:

```text
const displayResults = computed<SearchBook[]>(() => {
shelfStore.searchResults = [...shelfStore.searchResults, ...newBooks]
```

- [ ] **Step 2: Run frontend tests to catch type regressions**

```bash
cd /Users/maple/Documents/reader/frontend
npm test -- --run
```

Expected:

```text
Test Files  ... passed
Tests       ... passed
```

- [ ] **Step 3: Run frontend build**

```bash
cd /Users/maple/Documents/reader/frontend
npm run build
```

Expected:

```text
✓ built
```

- [ ] **Step 4: Commit Task 4 only if frontend files changed**

Default expected outcome: no frontend file changes and no commit.

---

### Task 5: End-to-end local verification

**Files:**
- No planned source edits.

- [ ] **Step 1: Run backend focused and broad checks**

```bash
cd /Users/maple/Documents/reader
cargo test search_relevance merge_search_results
cargo test
```

Expected:

```text
test result: ok
```

Existing warnings about dead code are acceptable if tests pass.

- [ ] **Step 2: Run diff hygiene**

```bash
cd /Users/maple/Documents/reader
git diff --check
```

Expected: no output.

- [ ] **Step 3: Manual verification with local app**

Start backend/frontend in the normal local setup. Search `没钱修什么仙` in “全部书源”. Expected behavior:

```text
Top results are exact/full-title matches.
Weak overlap titles such as “修什么仙造作啊” or “什么？我家老祖竟是仙帝？” do not appear while strong matches exist.
If a different query has no strong matches, the app still shows best available source results instead of emptying the page.
```

- [ ] **Step 4: Final commit if previous tasks were squashed manually**

If implementation was not committed task-by-task, create one focused commit:

```bash
cd /Users/maple/Documents/reader
git add src/service/mod.rs src/service/search_relevance.rs src/api/handlers/book.rs
git commit -m "feat: improve search result relevance"
```

---

## Risks and Constraints

- Chinese title relevance is heuristic. It will improve the obvious noisy cases but can still mis-rank titles with intentional punctuation, aliases, or very short names.
- Filtering happens after a book source returns data. It does not change remote book-source search quality or network cost.
- Batch-level SSE sorting cannot globally sort all sources before display without losing streaming behavior. It improves each batch and prevents weak batches from filling the result limit too early.
- If users want every raw result, add a future UI toggle named `显示低相关结果`; do not add it in this first pass.

## Self-Review

- Spec coverage: explains root cause, adds backend scoring, applies to JSON and SSE multi-search, keeps frontend simple, includes verification.
- Placeholder scan: no deferred code blocks or undefined helper names remain in this plan.
- Type consistency: all helper signatures use existing `SearchBook`; no frontend field is required.

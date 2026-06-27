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

    let query_chars: Vec<char> = query.chars().collect();
    let title_chars: Vec<char> = title.chars().collect();
    let query_len = query_chars.len().max(1);
    let common_count = query_chars
        .iter()
        .filter(|ch| title_chars.contains(ch))
        .count();
    let ordered_count = ordered_subsequence_match_count(&query_chars, &title_chars);

    let mut score = (common_count as i32) * 100 + (ordered_count as i32) * 25;
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
    } else if has_query_prefix(&query_chars, &title_chars, 2)
        && common_count * 100 / query_len >= 75
    {
        score += 4_500;
        strong_match = true;
    }

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

pub fn filter_strong_search_results(query: &str, books: Vec<SearchBook>) -> Vec<SearchBook> {
    if query.trim().is_empty() {
        return books;
    }

    sort_and_filter_search_results(query, books)
        .into_iter()
        .filter(|book| score_search_book(query, book).strong_match)
        .collect()
}

fn normalize_search_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| {
            !ch.is_whitespace()
                && !matches!(
                    ch,
                    '《' | '》'
                        | '「'
                        | '」'
                        | '“'
                        | '”'
                        | '"'
                        | '\''
                        | ':'
                        | '：'
                        | '-'
                        | '_'
                        | '，'
                        | ','
                        | '。'
                        | '.'
                        | '？'
                        | '?'
                        | '！'
                        | '!'
                        | '；'
                        | ';'
                )
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn has_query_prefix(query: &[char], title: &[char], min_len: usize) -> bool {
    query.len() >= min_len && title.len() >= min_len && query[..min_len] == title[..min_len]
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
    use super::{filter_strong_search_results, score_search_book, sort_and_filter_search_results};
    use crate::model::search::SearchBook;

    fn book(name: &str) -> SearchBook {
        SearchBook {
            name: name.to_string(),
            author: "作者".to_string(),
            origin: "https://source.test".to_string(),
            book_url: format!("https://source.test/{name}"),
            ..SearchBook::default()
        }
    }

    #[test]
    fn exact_title_scores_above_partial_noise() {
        let exact = score_search_book("没钱修什么仙", &book("没钱修什么仙"));
        let partial = score_search_book("没钱修什么仙", &book("修什么仙造作啊"));

        assert!(exact.strong_match);
        assert!(!partial.strong_match);
        assert!(exact.score > partial.score);
    }

    #[test]
    fn full_query_containment_is_strong() {
        let score = score_search_book("没钱修什么仙", &book("我在异界没钱修什么仙"));

        assert!(score.strong_match);
        assert!(score.score >= 8_000);
    }

    #[test]
    fn similar_compact_title_with_key_prefix_is_kept_but_lower() {
        let exact = score_search_book("没钱修什么仙", &book("没钱修什么仙"));
        let similar = score_search_book("没钱修什么仙", &book("没钱修仙是什么体验"));

        assert!(similar.strong_match);
        assert!(similar.score < exact.score);
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
                book("没钱修仙是什么体验"),
            ],
        );

        let names: Vec<String> = results.into_iter().map(|book| book.name).collect();
        assert_eq!(
            names,
            vec!["没钱修什么仙", "我在异界没钱修什么仙", "没钱修仙是什么体验"]
        );
    }

    #[test]
    fn weak_results_are_not_all_dropped_when_no_strong_match_exists() {
        let results = sort_and_filter_search_results(
            "不存在的冷门书名",
            vec![book("普通玄幻"), book("冷门修仙")],
        );

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn strict_filter_drops_all_weak_noise() {
        let results = filter_strong_search_results(
            "不存在的冷门书名",
            vec![book("普通玄幻"), book("冷门修仙")],
        );

        assert!(results.is_empty());
    }
}

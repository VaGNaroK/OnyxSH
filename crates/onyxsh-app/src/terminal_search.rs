//! Phase 9: Advanced VTE Terminal Search Engine (`Gtk.SearchBar` + `vte4::Regex` + Match Counter).
//! Ports search logic from `src/onyxsh/ui/window_ui.py`, `src/onyxsh/terminal/manager.py`, and `window.py`.

use regex::RegexBuilder;
use vte4::prelude::*;

/// PCRE2 flags used by `vte4::Regex::for_search`:
/// `PCRE2_UTF` (0x00080000) | `PCRE2_MULTILINE` (0x00000400)
const PCRE2_UTF_MULTILINE: u32 = 0x0008_0000 | 0x0000_0400;
/// `PCRE2_CASELESS` (0x00000008)
const PCRE2_CASELESS: u32 = 0x0000_0008;

/// Builds a regular expression pattern from the user's search query, honoring
/// `use_regex` (raw regex vs. escaped literal) and `whole_word` (`\b...\b`).
pub fn build_search_pattern(query: &str, use_regex: bool, whole_word: bool) -> String {
    if query.is_empty() {
        return String::new();
    }
    let base = if use_regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    if whole_word {
        format!(r"\b(?:{base})\b")
    } else {
        base
    }
}

/// Counts total non-overlapping occurrences of `query` inside `buffer_text` using the active
/// search options (`case_sensitive`, `whole_word`, `use_regex`).
pub fn count_buffer_occurrences(
    buffer_text: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
) -> usize {
    if query.is_empty() || buffer_text.is_empty() {
        return 0;
    }
    let pattern = build_search_pattern(query, use_regex, whole_word);
    let Ok(compiled) = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .multi_line(true)
        .build()
    else {
        return 0;
    };
    compiled.find_iter(buffer_text).count()
}

/// Formats the occurrence counter badge (`0/0` or `1 de 12` / `1/12`).
pub fn format_occurrence_counter(current_index_zero_based: usize, total_matches: usize) -> String {
    if total_matches == 0 {
        "0/0".to_string()
    } else {
        let clamped = (current_index_zero_based + 1).min(total_matches);
        format!("{clamped}/{total_matches}")
    }
}

/// Applies the search regex to a live `vte4::Terminal`, highlights matches, jumps to the first match,
/// and returns the total number of matches found in the terminal's visible text buffer.
pub fn apply_vte_search(
    terminal: &vte4::Terminal,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
) -> usize {
    if query.is_empty() {
        terminal.search_set_regex(None, 0);
        terminal.unselect_all();
        return 0;
    }

    let pattern = build_search_pattern(query, use_regex, whole_word);
    let mut flags = PCRE2_UTF_MULTILINE;
    if !case_sensitive {
        flags |= PCRE2_CASELESS;
    }

    if let Ok(vte_regex) = vte4::Regex::for_search(&pattern, flags) {
        terminal.search_set_regex(Some(&vte_regex), 0);
        terminal.search_set_wrap_around(true);
        terminal.search_find_next();
    } else {
        terminal.search_set_regex(None, 0);
        return 0;
    }

    let buffer_text = terminal
        .text_format(vte4::Format::Text)
        .map(|gstr| gstr.to_string())
        .unwrap_or_default();

    count_buffer_occurrences(&buffer_text, query, case_sensitive, whole_word, use_regex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_pattern_literal_whole_word_and_regex() {
        assert_eq!(build_search_pattern("a.b", false, false), r"a\.b");
        assert_eq!(build_search_pattern("err", false, true), r"\b(?:err)\b");
        assert_eq!(build_search_pattern("err[0-9]+", true, true), r"\b(?:err[0-9]+)\b");
    }

    #[test]
    fn test_count_buffer_occurrences_options() {
        let sample = "Error: failed to connect\nerror: retry 1\nERROR_CODE=42\n";
        // Case-insensitive, substring
        assert_eq!(
            count_buffer_occurrences(sample, "error", false, false, false),
            3
        );
        // Case-sensitive ("Error" only matches first line)
        assert_eq!(
            count_buffer_occurrences(sample, "Error", true, false, false),
            1
        );
        // Whole word + case-insensitive ("ERROR_CODE" excluded because of `_CODE`)
        assert_eq!(
            count_buffer_occurrences(sample, "error", false, true, false),
            2
        );
        // Regex match
        assert_eq!(
            count_buffer_occurrences(sample, r"retry \d+", false, false, true),
            1
        );
        assert_eq!(format_occurrence_counter(0, 3), "1/3");
        assert_eq!(format_occurrence_counter(0, 0), "0/0");
    }
}

/// Minimal case-insensitive glob used by the `name_matches` condition. `*` matches any run of
/// characters (including empty) and `?` matches exactly one. Kept deliberately tiny so the
/// engine takes no extra dependency for one small feature; it matches against a file name,
/// not a full path, so there is no special handling of `/`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let text: Vec<char> = text.to_ascii_lowercase().chars().collect();

    // Iterative wildcard match with backtracking: on a mismatch we fall back to the most
    // recent `*` and let it consume one more character.
    let (mut p, mut t) = (0usize, 0usize);
    let mut star_pattern: Option<usize> = None;
    let mut star_text = 0usize;

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star_pattern = Some(p);
            star_text = t;
            p += 1;
        } else if let Some(star) = star_pattern {
            p = star + 1;
            star_text += 1;
            t = star_text;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }

    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn matches_plain_text_case_insensitively() {
        assert!(glob_match("readme.md", "README.md"));
        assert!(!glob_match("readme.md", "notes.md"));
    }

    #[test]
    fn star_matches_any_run_including_empty() {
        assert!(glob_match("*invoice*", "2024-invoice-final.pdf"));
        assert!(glob_match("*invoice*", "invoice"));
        assert!(glob_match("report*", "report"));
        assert!(glob_match("*.tmp", "scratch.tmp"));
        assert!(!glob_match("*.tmp", "scratch.txt"));
    }

    #[test]
    fn question_mark_matches_exactly_one_char() {
        assert!(glob_match("log?.txt", "log1.txt"));
        assert!(!glob_match("log?.txt", "log.txt"));
        assert!(!glob_match("log?.txt", "log12.txt"));
    }

    #[test]
    fn handles_consecutive_stars_without_backtracking_blowup() {
        assert!(glob_match("a**b", "axxxxb"));
        assert!(!glob_match("a**b", "axxxxc"));
    }
}

//! Shaping recorded values for a terminal table.
//!
//! Patterns and errors are captured verbatim, so they arrive with whatever
//! newlines and length the original command had -- a heredoc or a long pipeline
//! will otherwise blow the table apart. Abbreviating is a display concern only;
//! the stored values stay untouched.

/// Longest a cell renders before it is cut. Wide enough to keep a command
/// recognisable, narrow enough that a few of them still fit side by side.
const MAX_LEN: usize = 60;

const ELLIPSIS: char = '…';

/// Flattens to one line and caps the length.
pub fn abbreviate(value: &str) -> String {
    let flattened = value.split_whitespace().collect::<Vec<_>>().join(" ");

    if flattened.chars().count() <= MAX_LEN {
        return flattened;
    }

    let mut out: String = flattened.chars().take(MAX_LEN - 1).collect();
    out.push(ELLIPSIS);
    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn collapses_newlines_into_one_line() {
        assert_eq!(abbreviate("python3 - <<'EOF'\nimport re\nEOF"), "python3 - <<'EOF' import re EOF");
    }

    #[test]
    fn leaves_ordinary_patterns_alone() {
        assert_eq!(abbreviate("git status --short"), "git status --short");
    }

    #[test]
    fn caps_long_values() {
        let out = abbreviate(&"x".repeat(200));
        assert_eq!(out.chars().count(), MAX_LEN);
        assert!(out.ends_with(ELLIPSIS));
    }
}

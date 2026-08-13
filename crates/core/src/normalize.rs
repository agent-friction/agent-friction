use regex::{Captures, Regex};
use std::sync::LazyLock;

static PATH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\w.~-]*(?:/[\w.-]+)+").unwrap());
static LONG_NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{4,}").unwrap());
static HEX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[0-9a-f]{8,}\b").unwrap());

const MAX_LEN: usize = 200;

pub(crate) fn normalize_error(error: &str) -> String {
    let s = PATH_RE.replace_all(error, |caps: &Captures| match caps[0].rsplit('/').next() {
        Some(base) if !base.is_empty() => format!("<path>/{base}"),
        _ => "<path>".to_string(),
    });
    let s = LONG_NUM_RE.replace_all(&s, "<n>");
    let s = HEX_RE.replace_all(&s, "<id>");
    s.chars().take(MAX_LEN).collect()
}

#[cfg(test)]
mod test {
    use crate::normalize::normalize_error;


    #[test]
    fn distinguishes_binaries_in_different_repos() {
        let a = normalize_error("exit 1: /Users/c/repos/foo/node_modules/.bin/jest failed");
        let b = normalize_error("exit 1: /Users/c/repos/bar/node_modules/.bin/jest failed");
        let c = normalize_error("exit 1: /Users/c/repos/foo/node_modules/.bin/cypress failed");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}

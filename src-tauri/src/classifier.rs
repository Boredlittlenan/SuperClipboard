use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use url::Url;

/// Content categories for clipboard items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Text,
    Link,
    Image,
    Code,
    Email,
    FilePath,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Text => write!(f, "text"),
            Category::Link => write!(f, "link"),
            Category::Image => write!(f, "image"),
            Category::Code => write!(f, "code"),
            Category::Email => write!(f, "email"),
            Category::FilePath => write!(f, "file_path"),
        }
    }
}

// Pre-compiled regex patterns for performance
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}$").unwrap()
});

static CODE_INDICATORS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^\s*(fn |func |def |class |import |from |const |let |var |public |private |#include|<\?php|<!DOCTYPE|<html|<div|<script|\{[\s]*$|=>\s*\{)"
    ).unwrap()
});

static WINDOWS_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[A-Za-z]:\\(.+\\)*[^\\/:*?"<>|]+$"#).unwrap()
});

static UNIX_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(/[^/\x00]+)+/?$").unwrap()
});

/// Classify text content into a category
pub fn classify_text(text: &str) -> Category {
    let trimmed = text.trim();

    // Empty or whitespace
    if trimmed.is_empty() {
        return Category::Text;
    }

    // Check for URL/Link
    if Url::parse(trimmed).is_ok()
        && (trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("ftp://"))
    {
        return Category::Link;
    }

    // Check for email
    if EMAIL_RE.is_match(trimmed) {
        return Category::Email;
    }

    // Check for file path
    if WINDOWS_PATH_RE.is_match(trimmed) || UNIX_PATH_RE.is_match(trimmed) {
        return Category::FilePath;
    }

    // Check for code snippets (multiline or contains code indicators)
    if trimmed.lines().count() > 1 && CODE_INDICATORS.is_match(trimmed) {
        return Category::Code;
    }

    // Check for JSON-like content
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        if trimmed.lines().count() > 1 {
            return Category::Code;
        }
    }

    Category::Text
}

/// Classify image content
pub fn classify_image() -> Category {
    Category::Image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_link() {
        assert_eq!(classify_text("https://example.com"), Category::Link);
        assert_eq!(
            classify_text("http://foo.bar/baz?q=1"),
            Category::Link
        );
    }

    #[test]
    fn test_classify_email() {
        assert_eq!(classify_text("user@example.com"), Category::Email);
    }

    #[test]
    fn test_classify_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        assert_eq!(classify_text(code), Category::Code);
    }

    #[test]
    fn test_classify_text() {
        assert_eq!(classify_text("Hello, world!"), Category::Text);
    }

    #[test]
    fn test_classify_windows_path() {
        assert_eq!(
            classify_text(r"C:\Users\test\file.txt"),
            Category::FilePath
        );
    }
}

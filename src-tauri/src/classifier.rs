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

// ─── Pre-compiled patterns ──────────────────────────────────────────

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}$").unwrap());

static WINDOWS_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[A-Za-z]:\\(.+\\)*[^\\/:*?"<>|]+$"#).unwrap());

static UNIX_PATH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(/[^/\x00]+)+/?$").unwrap());

/// Bare domain detection: www.example.com, example.com, sub.domain.co.uk, etc.
/// Matches optional www. prefix, domain labels, and a TLD of 2+ characters.
/// Allows paths, query strings, and fragments after the domain.
static DOMAIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(www\.)?([a-zA-Z0-9\-]+\.)+[a-zA-Z]{2,}(/[^\s]*)?$").unwrap());

static EMBEDDED_EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}\b").unwrap());

static EMBEDDED_WINDOWS_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[A-Za-z]:\\[^\r\n<>|"]+"#).unwrap());

static EMBEDDED_UNIX_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)(?:^|\s)(/(?:Users|home|var|etc|tmp|mnt|Volumes)/[^\s]+)").unwrap()
});

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:https?|ftp)://[^\s]+").unwrap());

// ─── Code detection patterns (used for scoring) ─────────────────────

/// Keywords that strongly indicate code (weight: 3 each)
static CODE_KEYWORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)\b(fn|func|function|def|class|struct|enum|interface|trait|impl|module|package|import|export|from|require|include|const|let|var|val|mut|public|private|protected|async|await|return|throw|try|catch|finally|match|switch|foreach|typeof|instanceof)\b|#include|<\?php|<!DOCTYPE|<html|@media|@keyframes|@import").unwrap()
});

/// Syntax patterns that indicate code (weight: 2 each)
static CODE_SYNTAX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"=>\s*[\{\(]|->\s*[\{\(]|\b\w+\s*::\s*\w+|\.\w+\s*\([^)]*\)|\bnew\s+\w+").unwrap()
});

/// CSS property declarations: `property: value;` or `property: value}`
static CSS_PROPERTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(margin|padding|width|height|display|flex|grid|position|color|background|border|font-size|font-weight|font-family|text-align|text-decoration|overflow|transform|transition|animation|justify-content|align-items|flex-direction|box-shadow|border-radius|z-index|opacity|cursor|outline|top|left|right|bottom|min-width|max-width|min-height|max-height|line-height|letter-spacing|white-space|word-break|vertical-align|float|clear|visibility|content|src|gap|object-fit|pointer-events|user-select|box-sizing|list-style)\s*:\s*[^;}{]+[;}]").unwrap()
});

/// CSS selector block: `selector { ... }`
static CSS_SELECTOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[.#][\w\-]+\s*\{[^}]*\}|[\w\-]+\s*\{[^}]*:[^}]*\}").unwrap());

/// HTML/XML tags
static HTML_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<\/?[a-zA-Z][\w\-]*(\s+[\w\-]+(="[^"]*")?)*\s*/?\s*>|<\w+\s[^>]+>"#).unwrap()
});

/// SQL keywords
static SQL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(SELECT|INSERT INTO|UPDATE.*SET|DELETE FROM|CREATE TABLE|ALTER TABLE|DROP TABLE|FROM\s+\w+|WHERE\s+\w+|JOIN\s+\w+|GROUP BY|ORDER BY|HAVING|UNION|INDEX)\b").unwrap()
});

/// Shell/command patterns
static SHELL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*\$\s+\w+|#!/bin/|\b(sudo|apt-get|apt|npm|pnpm|yarn|pip|cargo|docker|git|curl|wget|chmod|chown|mkdir)\b").unwrap()
});

/// Regex literal patterns (e.g. /\w+/g, /^test$/i)
static REGEX_LITERAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/[^/\n]+/[gimsuy]{0,6}").unwrap());

/// Markdown indicators: # headers, **bold**, [text](url), ```code blocks```, - list items
static MARKDOWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^#{1,6}\s+\S|^\s*[-*]\s+\S|\*\*\S[^*]+\*\*|`{3}|\[[^\]]+\]\([^)]+\)|^\s*>\s+\S",
    )
    .unwrap()
});

/// YAML/TOML indicators: key: value lines (without braces), [section] headers
static CONFIG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\w][\w.-]*:\s+\S|^\[[\w][\w.-]*\]$").unwrap());

/// Generic code structure
static CODE_STRUCTURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?m)\{[\s]*$|^[\s]*\}|;\s*$|//[^\n]*$|/\*.*\*/"#).unwrap());

/// JSON with quoted keys: `"key":` or `"key" :`
static JSON_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""[a-zA-Z_][\w]*"\s*:"#).unwrap());

// Threshold to classify as code
const CODE_SCORE_THRESHOLD: i32 = 5;

fn trim_token_punctuation(token: &str) -> &str {
    token.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
        )
    })
}

fn is_link_token(token: &str) -> bool {
    let trimmed = trim_token_punctuation(token);
    if trimmed.is_empty() {
        return false;
    }

    let has_scheme = Url::parse(trimmed).is_ok()
        && (trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("ftp://"));
    let is_bare_domain = !trimmed.contains(' ') && DOMAIN_RE.is_match(trimmed);
    has_scheme || is_bare_domain
}

fn looks_like_json_code(trimmed: &str) -> bool {
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        return JSON_KEY_RE.is_match(trimmed) || (trimmed.contains(',') && trimmed.contains(':'));
    }
    false
}

pub fn contains_email(text: &str) -> bool {
    EMBEDDED_EMAIL_RE.is_match(text)
}

pub fn contains_link(text: &str) -> bool {
    URL_RE.is_match(text) || text.split_whitespace().any(is_link_token)
}

pub fn contains_file_path(text: &str) -> bool {
    EMBEDDED_WINDOWS_PATH_RE.is_match(text)
        || EMBEDDED_UNIX_PATH_RE.is_match(text)
        || text.split_whitespace().any(|token| {
            let trimmed = trim_token_punctuation(token);
            WINDOWS_PATH_RE.is_match(trimmed)
                || trimmed.starts_with("./")
                || trimmed.starts_with("../")
                || trimmed.starts_with("~/")
        })
}

pub fn contains_code(text: &str) -> bool {
    let trimmed = text.trim();
    let code_scoring_text = strip_non_code_tokens(trimmed);
    looks_like_json_code(trimmed) || score_code(&code_scoring_text) >= CODE_SCORE_THRESHOLD
}

fn strip_non_code_tokens(text: &str) -> String {
    let without_urls = URL_RE.replace_all(text, " ");
    let without_emails = EMBEDDED_EMAIL_RE.replace_all(&without_urls, " ");
    let without_windows_paths = EMBEDDED_WINDOWS_PATH_RE.replace_all(&without_emails, " ");
    let without_unix_paths = EMBEDDED_UNIX_PATH_RE.replace_all(&without_windows_paths, " ");
    without_unix_paths.to_string()
}

fn has_plain_text_after_structured_tokens(text: &str) -> bool {
    text.lines().any(|line| {
        let sanitized = strip_non_code_tokens(line);
        let trimmed = sanitized.trim();
        has_word_like_char(trimmed) && !looks_like_code_line(trimmed)
    })
}

fn has_word_like_char(text: &str) -> bool {
    text.chars()
        .any(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&c))
}

fn looks_like_code_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if matches!(trimmed, "{" | "}" | ");" | "};") {
        return true;
    }
    if trimmed.contains("/*") || trimmed.contains("*/") || trimmed.starts_with("//") {
        return true;
    }
    if CSS_PROPERTY_RE.is_match(trimmed) || CSS_SELECTOR_RE.is_match(trimmed) {
        return true;
    }
    if trimmed.ends_with('{') && (trimmed.contains('.') || trimmed.contains('#')) {
        return true;
    }
    let keyword_matches = CODE_KEYWORDS.find_iter(trimmed).take(1).count();
    let syntax_matches = CODE_SYNTAX.find_iter(trimmed).take(1).count();
    keyword_matches > 0 || syntax_matches > 0
}

fn push_unique_category(categories: &mut Vec<Category>, category: Category) {
    if !categories.contains(&category) {
        categories.push(category);
    }
}

/// Return every category signal detected for a text clipboard entry.
pub fn classify_text_tags(text: &str) -> Vec<Category> {
    let mut categories = Vec::new();
    let primary = classify_text(text);

    if contains_link(text) {
        push_unique_category(&mut categories, Category::Link);
    }
    if contains_email(text) {
        push_unique_category(&mut categories, Category::Email);
    }
    if contains_file_path(text) {
        push_unique_category(&mut categories, Category::FilePath);
    }
    if contains_code(text) {
        push_unique_category(&mut categories, Category::Code);
    }
    if has_plain_text_after_structured_tokens(text) {
        push_unique_category(&mut categories, Category::Text);
    } else if primary != Category::Text {
        push_unique_category(&mut categories, primary);
    }

    if categories.is_empty() {
        categories.push(Category::Text);
    }
    categories
}

/// Score the content for code-likeness and classify
fn score_code(text: &str) -> i32 {
    let mut score: i32 = 0;
    let line_count = text.lines().count();

    // Strong keywords: 3 points each, max 9
    let kw_matches: usize = CODE_KEYWORDS.find_iter(text).take(3).count();
    score += kw_matches as i32 * 3;

    // Syntax patterns: 2 points each, max 6
    let syn_matches: usize = CODE_SYNTAX.find_iter(text).take(3).count();
    score += syn_matches as i32 * 2;

    // CSS property declarations: 2 points each, max 6
    let css_prop_matches: usize = CSS_PROPERTY_RE.find_iter(text).take(3).count();
    score += css_prop_matches as i32 * 2;

    // CSS selector blocks: 3 points (strong signal)
    if CSS_SELECTOR_RE.is_match(text) {
        score += 3;
    }

    // HTML tags: 2 points each, max 4
    let html_matches: usize = HTML_TAG_RE.find_iter(text).take(2).count();
    score += html_matches as i32 * 2;

    // SQL keywords: 2 points each, max 4
    let sql_matches: usize = SQL_RE.find_iter(text).take(2).count();
    score += sql_matches as i32 * 2;

    // Shell patterns: 2 points each, max 4
    let shell_matches: usize = SHELL_RE.find_iter(text).take(2).count();
    score += shell_matches as i32 * 2;

    // Regex literals: 2 points each, max 4
    let regex_matches: usize = REGEX_LITERAL_RE.find_iter(text).take(2).count();
    score += regex_matches as i32 * 2;

    // Markdown patterns: 2 points each, max 6
    let md_matches: usize = MARKDOWN_RE.find_iter(text).take(3).count();
    score += md_matches as i32 * 2;

    // YAML/TOML config patterns: 2 points each, max 4
    let config_matches: usize = CONFIG_RE.find_iter(text).take(2).count();
    score += config_matches as i32 * 2;

    // Generic code structure: 1 point each, max 5
    let struct_matches: usize = CODE_STRUCTURE.find_iter(text).take(5).count();
    score += struct_matches as i32;

    // Multi-line bonus: +2 if more than 1 line
    if line_count > 1 {
        score += 2;
    }

    // Heavy multi-line bonus: +3 if more than 5 lines with structure
    if line_count > 5 && struct_matches >= 2 {
        score += 3;
    }

    // Penalty: if text looks like natural language (no structure markers)
    // Only applied to single-line or very short text
    let has_semicolon = text.contains(';');
    let has_braces = text.contains('{') || text.contains('}');
    if !has_semicolon
        && !has_braces
        && line_count == 1
        && kw_matches == 0
        && css_prop_matches == 0
        && html_matches == 0
        && !CSS_SELECTOR_RE.is_match(text)
        && regex_matches == 0
        && md_matches == 0
        && config_matches == 0
    {
        score -= 2;
    }

    score
}

/// Classify text content into a category
pub fn classify_text(text: &str) -> Category {
    let trimmed = text.trim();

    // Empty or whitespace
    if trimmed.is_empty() {
        return Category::Text;
    }

    // Check for URL/Link (must be a single URL, not embedded in other text)
    if trimmed.lines().count() <= 2 {
        // Standard scheme-based detection
        if is_link_token(trimmed) {
            return Category::Link;
        }
    }

    // Check for email
    if EMAIL_RE.is_match(trimmed) {
        return Category::Email;
    }

    // Check for file path (single line only)
    if trimmed.lines().count() == 1
        && (WINDOWS_PATH_RE.is_match(trimmed) || UNIX_PATH_RE.is_match(trimmed))
    {
        return Category::FilePath;
    }

    // Check for JSON-like content (stricter: require quoted key)
    if looks_like_json_code(trimmed) {
        return Category::Code;
    }

    // Score-based code detection
    if contains_code(trimmed) {
        return Category::Code;
    }

    Category::Text
}

/// Classify image content
pub fn classify_image() -> Category {
    Category::Image
}

pub fn classify_image_tags() -> Vec<Category> {
    vec![Category::Image]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_link() {
        assert_eq!(classify_text("https://example.com"), Category::Link);
        assert_eq!(classify_text("http://foo.bar/baz?q=1"), Category::Link);
    }

    #[test]
    fn test_classify_email() {
        assert_eq!(classify_text("user@example.com"), Category::Email);
    }

    #[test]
    fn test_classify_code_rust() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        assert_eq!(classify_text(code), Category::Code);
    }

    #[test]
    fn test_classify_code_css_multiline() {
        let css = "figure.elementor-image-box-img {\n  width: 100% !important;\n  display: flex;\n  justify-content: center;\n}";
        assert_eq!(
            classify_text(css),
            Category::Code,
            "CSS multi-line should be code"
        );
    }

    #[test]
    fn test_classify_code_css_inline() {
        let css = ".container { display: flex; align-items: center; }";
        assert_eq!(
            classify_text(css),
            Category::Code,
            "CSS inline should be code"
        );
    }

    #[test]
    fn test_classify_code_css_property() {
        let css = "justify-content: center;\ndisplay: flex;\nmargin: 0 auto;";
        assert_eq!(
            classify_text(css),
            Category::Code,
            "CSS properties should be code"
        );
    }

    #[test]
    fn test_classify_code_js() {
        let js = "const arr = [1, 2, 3].map(x => x * 2);\nconsole.log(arr);";
        assert_eq!(classify_text(js), Category::Code, "JS should be code");
    }

    #[test]
    fn test_classify_code_python() {
        let py = "def hello():\n    print(\"world\")\n    return True";
        assert_eq!(classify_text(py), Category::Code, "Python should be code");
    }

    #[test]
    fn test_classify_code_html() {
        let html = "<div class=\"container\">\n  <h1>Hello</h1>\n  <p>World</p>\n</div>";
        assert_eq!(classify_text(html), Category::Code, "HTML should be code");
    }

    #[test]
    fn test_classify_json() {
        let json = "{\n  \"name\": \"test\",\n  \"version\": \"1.0\"\n}";
        assert_eq!(classify_text(json), Category::Code, "JSON should be code");
    }

    #[test]
    fn test_classify_json_not_false_positive() {
        // Simple braces without quoted keys should not be code
        assert_ne!(classify_text("{smile}"), Category::Code);
        assert_ne!(classify_text("[test]"), Category::Code);
    }

    #[test]
    fn test_classify_markdown() {
        let md = "# Hello World\n\nThis is **bold** text.\n\n- item one\n- item two";
        assert_eq!(classify_text(md), Category::Code, "Markdown should be code");
    }

    #[test]
    fn test_classify_yaml() {
        let yaml = "name: my-app\nversion: 1.0.0\ndescription: A test app\nport: 3000";
        assert_eq!(classify_text(yaml), Category::Code, "YAML should be code");
    }

    #[test]
    fn test_classify_toml() {
        let toml = "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"";
        assert_eq!(classify_text(toml), Category::Code, "TOML should be code");
    }

    #[test]
    fn test_classify_text() {
        assert_eq!(classify_text("Hello, world!"), Category::Text);
        assert_eq!(classify_text("这是一段普通的中文文本"), Category::Text);
        assert_eq!(
            classify_text("The quick brown fox jumps over the lazy dog"),
            Category::Text
        );
    }

    #[test]
    fn test_classify_windows_path() {
        assert_eq!(classify_text(r"C:\Users\test\file.txt"), Category::FilePath);
    }

    #[test]
    fn test_natural_text_not_code() {
        assert_ne!(classify_text("今天天气不错，适合出去走走"), Category::Code);
        assert_ne!(
            classify_text("Check out the documentation for more info"),
            Category::Code
        );
    }

    #[test]
    fn test_classify_regex_literal() {
        let code = "const pattern = /\\bhello\\b/gi;\nconst match = text.match(pattern);";
        assert_eq!(
            classify_text(code),
            Category::Code,
            "Regex literal should be code"
        );
    }

    #[test]
    fn test_embedded_content_signals() {
        assert!(contains_email("Contact me at user@example.com tomorrow"));
        assert!(contains_link("Docs: https://example.com/guide"));
        assert!(contains_link("Open www.example.com for details"));
        assert!(contains_file_path(r"Saved at C:\Users\test\file.txt"));
        assert!(contains_file_path(
            "Config lives in /Users/test/app/config.json"
        ));
        assert!(contains_code("const value = items.map(item => item.id);"));
    }

    #[test]
    fn test_classify_text_tags_for_mixed_content() {
        let text = r"Send logs to user@example.com and save C:\Users\test\log.txt";
        let tags = classify_text_tags(text);
        assert!(tags.contains(&Category::Text));
        assert!(tags.contains(&Category::Email));
        assert!(tags.contains(&Category::FilePath));
    }

    #[test]
    fn test_link_and_email_lines_are_not_code() {
        let text = "https://ajiro.infini-cloud.net/dav/\n\n123@123.com";
        assert_eq!(classify_text(text), Category::Text);
        let tags = classify_text_tags(text);
        assert!(!tags.contains(&Category::Text));
        assert!(tags.contains(&Category::Link));
        assert!(tags.contains(&Category::Email));
        assert!(!tags.contains(&Category::Code));
    }

    #[test]
    fn test_mixed_structured_code_and_plain_lines_keep_text() {
        let text = "https://ajiro.infini-cloud.net/dav/\n\n123@123.com\nE:\\Code\\Python\\BulkEmailSender111\n666666\n\n.banner2-title-container .wd-text-block {\n    color: #e0e0e0; /* 白灰色 */\n}\n\n99999";
        let tags = classify_text_tags(text);
        assert!(tags.contains(&Category::Link));
        assert!(tags.contains(&Category::Email));
        assert!(tags.contains(&Category::FilePath));
        assert!(tags.contains(&Category::Code));
        assert!(tags.contains(&Category::Text));
    }

    #[test]
    fn test_css_comment_is_not_file_path() {
        let text = ".banner2-title-container .wd-text-block {\n    color: #e0e0e0; /* 白灰色 */\n}";
        let tags = classify_text_tags(text);
        assert!(tags.contains(&Category::Code));
        assert!(!tags.contains(&Category::FilePath));
        assert!(!tags.contains(&Category::Text));
    }
}

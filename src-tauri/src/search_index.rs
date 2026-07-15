use crate::classifier::Category;
use regex::Regex;
use std::sync::LazyLock;

static MARKDOWN_IMAGE_DATA_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!\[[^\]]*\]\(data:image/[^)]*\)").expect("valid memo image regex")
});
static IMAGE_DATA_URI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"data:image/[A-Za-z0-9.+-]+;base64,[A-Za-z0-9+/=]+")
        .expect("valid image data URI regex")
});
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("valid whitespace regex"));

pub fn clipboard_search_text(
    content_type: &str,
    content: &str,
    preview: &str,
    category_tags: &[Category],
) -> String {
    let mut parts = Vec::with_capacity(category_tags.len() + 2);
    if !content_type.starts_with("image/") {
        parts.push(content.to_string());
    }
    parts.push(preview.to_string());
    parts.extend(
        category_tags
            .iter()
            .map(|category| category_aliases(category).to_string()),
    );
    normalize(&parts.join(" "))
}

pub fn memo_search_text(title: &str, body: &str, tags: &str, auto_tags: &[String]) -> String {
    let readable_body = strip_image_data(body);
    let mut parts = vec![title.to_string(), readable_body, tags.to_string()];
    parts.extend(auto_tags.iter().cloned());
    for tag in auto_tags {
        if let Some(aliases) = tag_aliases(tag) {
            parts.push(aliases.to_string());
        }
    }
    normalize(&parts.join(" "))
}

pub fn strip_image_data(value: &str) -> String {
    let without_markdown = MARKDOWN_IMAGE_DATA_RE.replace_all(value, " ");
    IMAGE_DATA_URI_RE
        .replace_all(without_markdown.as_ref(), " ")
        .into_owned()
}

fn category_aliases(category: &Category) -> &'static str {
    match category {
        Category::Text => "text 文本",
        Category::Link => "link url 链接",
        Category::Image => "image 图片",
        Category::Code => "code 代码",
        Category::Email => "email 邮箱",
        Category::FilePath => "file path 文件 路径",
    }
}

fn tag_aliases(tag: &str) -> Option<&'static str> {
    match tag.trim().to_ascii_lowercase().as_str() {
        "text" => Some("text 文本"),
        "link" | "url" => Some("link url 链接"),
        "image" => Some("image 图片"),
        "code" => Some("code 代码"),
        "email" => Some("email 邮箱"),
        "file" | "path" | "file_path" => Some("file path 文件 路径"),
        _ => None,
    }
}

fn normalize(value: &str) -> String {
    WHITESPACE_RE.replace_all(value.trim(), " ").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_images_exclude_base64_but_keep_metadata() {
        let search_text = clipboard_search_text(
            "image/png",
            "iVBORw0KGgoAAAANSUhEUgAAAAUA",
            "[Image 640x480]",
            &[Category::Image],
        );

        assert!(!search_text.contains("iVBORw0KGgo"));
        assert!(search_text.contains("640x480"));
        assert!(search_text.contains("图片"));
    }

    #[test]
    fn text_clipboard_entries_keep_content_and_category_aliases() {
        let search_text = clipboard_search_text(
            "text/plain",
            "https://example.com contact@example.com",
            "https://example.com",
            &[Category::Link, Category::Email],
        );

        assert!(search_text.contains("contact@example.com"));
        assert!(search_text.contains("链接"));
        assert!(search_text.contains("邮箱"));
    }

    #[test]
    fn memo_images_exclude_data_uri_but_keep_readable_text_and_tags() {
        let search_text = memo_search_text(
            "Design notes",
            "before\n![image](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA)\nafter",
            "work",
            &["image".to_string()],
        );

        assert!(!search_text.contains("iVBORw0KGgo"));
        assert!(search_text.contains("before after"));
        assert!(search_text.contains("Design notes"));
        assert!(search_text.contains("work"));
        assert!(search_text.contains("图片"));
    }
}

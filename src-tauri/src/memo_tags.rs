use crate::{
    classifier::{self, Category},
    search_index,
};

const GENERATED_TAG_ALIASES: &[&str] = &[
    "image", "图片", "email", "邮箱", "path", "路径", "link", "链接", "code", "代码",
];

fn is_generated_label(tag: &str) -> bool {
    GENERATED_TAG_ALIASES
        .iter()
        .any(|alias| tag.eq_ignore_ascii_case(alias) || tag == *alias)
}

pub fn manual_only(tags: &str) -> String {
    let mut result = Vec::new();
    for tag in tags.split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
        if is_generated_label(tag)
            || result
                .iter()
                .any(|existing: &&str| existing.eq_ignore_ascii_case(tag))
        {
            continue;
        }
        result.push(tag);
    }
    result.join(",")
}

fn body_has_image(body: &str) -> bool {
    body.contains("![image](data:image/")
        || body.contains("![image](http://")
        || body.contains("![image](https://")
        || body.contains("data:image/")
}

fn tag_from_category(category: &Category) -> Option<&'static str> {
    match category {
        Category::Link => Some("link"),
        Category::Email => Some("email"),
        Category::FilePath => Some("path"),
        Category::Code => Some("code"),
        _ => None,
    }
}

pub fn infer(title: &str, body: &str) -> Vec<String> {
    // Image Data URIs are storage payloads, not memo text. Classifying their Base64 bytes
    // can falsely look like code and produce an irrelevant automatic tag.
    let readable_body = search_index::strip_image_data(body);
    let content = format!("{title}\n{readable_body}");
    let mut tags = Vec::new();

    if body_has_image(body) {
        tags.push("image".to_string());
    }

    for category in classifier::classify_text_tags(&content) {
        if let Some(tag) = tag_from_category(&category) {
            if !tags.iter().any(|value| value == tag) {
                tags.push(tag.to_string());
            }
        }
    }

    tags
}

#[tauri::command]
pub fn infer_memo_tag_types(title: String, body: String) -> Vec<String> {
    infer(&title, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_clipboard_classifier_without_false_code() {
        let tags = infer(
            "555",
            "5555 222@123.com https://www.diskgenius.cn/help/restorefile\nhttps://v2rayn.co/",
        );

        assert!(tags.contains(&"email".to_string()));
        assert!(tags.contains(&"link".to_string()));
        assert!(!tags.contains(&"code".to_string()));
    }

    #[test]
    fn keeps_image_signal() {
        let tags = infer("", "note\n![image](data:image/png;base64,abc)");
        assert_eq!(tags.first().map(String::as_str), Some("image"));
    }

    #[test]
    fn image_data_does_not_create_a_code_tag() {
        let tags = infer(
            "Merged images (2)",
            "![image](data:image/png;base64,aGVsbG8rLz0=)\n![image](data:image/png;base64,Zm9vYmFyKys=)",
        );

        assert_eq!(tags, vec!["image"]);
    }

    #[test]
    fn separates_manual_tags_from_localized_auto_tag_labels() {
        assert_eq!(
            manual_only("project,EMAIL,邮箱,Link,链接,CODE,代码,Project"),
            "project"
        );
    }
}

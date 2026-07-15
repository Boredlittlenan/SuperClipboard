use crate::classifier::{self, Category};

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
    let content = format!("{title}\n{body}");
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
}

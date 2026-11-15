//! PS4 application metadata: the `param.json` GoldHEN keeps beside every
//! installed title in `/user/appmeta/<TITLE_ID>`.
//!
//! The shape differs between firmware, GoldHEN builds and fake PKG tools, so
//! every field is read by trying the known key names before giving up.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct TitleInfo {
    pub title_id: Option<String>,
    pub title: Option<String>,
    pub content_id: Option<String>,
    pub version: Option<String>,
    /// `gd` (game), `gp` (patch), `ac` (DLC), `theme`, `homebrew`.
    pub category: String,
    pub format: Option<String>,
    pub pkg_type: Option<String>,
}

/// Parse a `param.json` body. Unknown shapes return defaults instead of an
/// error: a title with no metadata still belongs in the library.
pub fn parse(body: &str) -> TitleInfo {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return TitleInfo::default();
    };
    let text = |keys: &[&str]| -> Option<String> {
        keys.iter()
            .filter_map(|key| value.get(key))
            .find_map(|v| match v {
                serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                serde_json::Value::Number(n) if n.is_u64() => Some(n.to_string()),
                _ => None,
            })
    };

    let content_id = text(&["contentId", "content_id"]);
    let mut title_id = text(&["titleId", "title_id", "TITLE_ID"]);
    if title_id.is_none() {
        title_id = content_id.as_deref().and_then(title_id_from_content_id);
    }
    let category = text(&["category", "contentType"])
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or_default();
    let pkg_type = text(&["pkgType", "pkg_type"]);
    let homebrew = category.is_empty()
        && (content_id.as_deref().unwrap_or("").starts_with("IV")
            || pkg_type.as_deref() == Some("fake"));

    TitleInfo {
        title_id,
        title: text(&["titleName", "title_name", "title", "TITLE"]),
        content_id,
        version: text(&["version", "masterVersion", "appVer"]),
        category: if category.is_empty() {
            if homebrew {
                "homebrew".into()
            } else {
                "app".into()
            }
        } else {
            category
        },
        format: text(&["format"]),
        pkg_type,
    }
}

/// `UP0001-CUSA12345_00-LABEL` -> `CUSA12345`.
pub fn title_id_from_content_id(content_id: &str) -> Option<String> {
    let rest = content_id.split_once('-')?.1;
    let id = rest.split('_').next()?.trim();
    if is_title_id(id) {
        Some(id.to_string())
    } else {
        None
    }
}

/// `CUSA12345`, `PPSA01234`, `XXXX00000`: four letters then five digits.
pub fn is_title_id(text: &str) -> bool {
    text.len() == 9
        && text[..4].bytes().all(|b| b.is_ascii_uppercase())
        && text[4..].bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_game_param() {
        let info = parse(
            r#"{"titleId":"CUSA07410","titleName":"God of War","contentId":
            "UP9000-CUSA07410_00-0000000000000000","version":"01.00",
            "category":"gd","format":"obs","pkgType":"fake"}"#,
        );
        assert_eq!(info.title.as_deref(), Some("God of War"));
        assert_eq!(info.title_id.as_deref(), Some("CUSA07410"));
        assert_eq!(info.version.as_deref(), Some("01.00"));
        assert_eq!(info.category, "gd");
    }

    #[test]
    fn falls_back_to_the_content_id() {
        let info = parse(r#"{"titleName":"Homebrew","contentId":"IV0000-HBOM00001_00-ABC"}"#);
        assert_eq!(info.title_id.as_deref(), Some("HBOM00001"));
        assert_eq!(info.category, "homebrew");
    }

    #[test]
    fn tolerates_garbage() {
        assert_eq!(parse("not json").title, None);
        assert_eq!(parse("{}").title_id, None);
    }

    #[test]
    fn recognises_title_ids() {
        assert!(is_title_id("CUSA07410"));
        assert!(!is_title_id("CUSA7410"));
        assert!(!is_title_id("cusa07410"));
    }
}

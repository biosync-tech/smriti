use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Represents a parsed wiki-link from markdown content
#[derive(Debug, Clone, PartialEq)]
pub struct WikiLink {
    /// The target note title
    pub target: String,
    /// Optional display alias (from [[target|alias]] syntax)
    pub alias: Option<String>,
    /// Optional section anchor (from [[target#section]] syntax)
    pub section: Option<String>,
    /// The full raw text matched
    pub raw: String,
    /// Inferred relation type from [[rel:TYPE|Target]] syntax (default: "wikilink")
    pub relation: String,
}

// Precompiled regex patterns for performance
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]\|#]+)(?:#([^\]\|]+))?(?:\|([^\]]+))?\]\]").unwrap());

// Typed wiki-link: [[rel:TYPE|Target]] or [[rel:TYPE|Target|Display]]
// Examples: [[rel:causal|Climate Change]], [[rel:semantic|AI Safety|related reading]]
static TYPED_WIKILINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\[rel:([a-zA-Z_][\w]*)\|([^\]\|]+)(?:\|([^\]]+))?\]\]").unwrap()
});

static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)#([a-zA-Z_][\w/-]*)").unwrap());

/// Extract all wiki-links from markdown content
///
/// Supports:
/// - `[[Note Title]]` — basic link (relation: wikilink)
/// - `[[Note Title|Display Text]]` — aliased link (relation: wikilink)
/// - `[[Note Title#Section]]` — section link (relation: wikilink)
/// - `[[Note Title#Section|Display]]` — section + alias (relation: wikilink)
/// - `[[rel:causal|Note Title]]` — typed link (relation: causal)
/// - `[[rel:semantic|Note Title|Display]]` — typed + alias (relation: semantic)
pub fn extract_wikilinks(content: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();

    // First pass: typed wiki-links [[rel:TYPE|Target]]
    // Collect their raw strings so we skip them in the plain pass.
    let mut typed_raws: HashSet<String> = HashSet::new();

    for cap in TYPED_WIKILINK_RE.captures_iter(content) {
        let raw = cap[0].to_string();
        typed_raws.insert(raw.clone());
        links.push(WikiLink {
            target: cap[2].trim().to_string(),
            alias: cap.get(3).map(|m| m.as_str().trim().to_string()),
            section: None,
            raw,
            relation: cap[1].trim().to_lowercase(),
        });
    }

    // Second pass: plain wiki-links (skip anything already matched as typed)
    for cap in WIKILINK_RE.captures_iter(content) {
        let raw = cap[0].to_string();
        if typed_raws.contains(&raw) {
            continue;
        }
        links.push(WikiLink {
            target: cap[1].trim().to_string(),
            alias: cap.get(3).map(|m| m.as_str().trim().to_string()),
            section: cap.get(2).map(|m| m.as_str().trim().to_string()),
            raw,
            relation: "wikilink".to_string(),
        });
    }

    links
}

/// Extract all hashtag-style tags from content
///
/// Matches: #tag, #my-tag, #nested/tag
/// Does NOT match: #123 (must start with a letter)
pub fn extract_tags(content: &str) -> Vec<String> {
    TAG_RE
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// Replace wiki-links in content with a custom format
/// Useful for rendering or converting to other formats
pub fn replace_wikilinks<F>(content: &str, replacer: F) -> String
where
    F: Fn(&WikiLink) -> String,
{
    let links = extract_wikilinks(content);
    let mut result = content.to_string();
    for link in links.iter().rev() {
        let replacement = replacer(link);
        result = result.replace(&link.raw, &replacement);
    }
    result
}

/// Convert wiki-links to standard markdown links
pub fn wikilinks_to_markdown(content: &str) -> String {
    replace_wikilinks(content, |link| {
        let display = link.alias.as_ref().unwrap_or(&link.target);
        let target_slug = link.target.to_lowercase().replace(' ', "-");
        format!("[{}](/notes/{})", display, target_slug)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_wikilink() {
        let links = extract_wikilinks("Check out [[My Note]] for details");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "My Note");
        assert_eq!(links[0].alias, None);
    }

    #[test]
    fn test_aliased_wikilink() {
        let links = extract_wikilinks("See [[Project Plan|the plan]] here");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Project Plan");
        assert_eq!(links[0].alias, Some("the plan".to_string()));
    }

    #[test]
    fn test_section_wikilink() {
        let links = extract_wikilinks("Jump to [[Guide#Installation]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Guide");
        assert_eq!(links[0].section, Some("Installation".to_string()));
    }

    #[test]
    fn test_multiple_wikilinks() {
        let links = extract_wikilinks("Link to [[A]] and [[B]] and [[C|see C]]");
        assert_eq!(links.len(), 3);
    }

    #[test]
    fn test_extract_tags() {
        let tags = extract_tags("This is #rust and #ai-agents with #nested/path");
        assert_eq!(tags, vec!["rust", "ai-agents", "nested/path"]);
    }

    #[test]
    fn test_no_numeric_tags() {
        let tags = extract_tags("Issue #123 is not a tag but #valid is");
        assert_eq!(tags, vec!["valid"]);
    }

    #[test]
    fn test_wikilinks_to_markdown() {
        let result = wikilinks_to_markdown("See [[My Note]] for info");
        assert_eq!(result, "See [My Note](/notes/my-note) for info");
    }

    #[test]
    fn test_basic_wikilink_has_wikilink_relation() {
        let links = extract_wikilinks("See [[My Note]]");
        assert_eq!(links[0].relation, "wikilink");
    }

    #[test]
    fn test_typed_wikilink_causal() {
        let links = extract_wikilinks("Due to [[rel:causal|Climate Change]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Climate Change");
        assert_eq!(links[0].relation, "causal");
        assert_eq!(links[0].alias, None);
    }

    #[test]
    fn test_typed_wikilink_semantic_with_alias() {
        let links = extract_wikilinks("See [[rel:semantic|AI Safety|related reading]]");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "AI Safety");
        assert_eq!(links[0].relation, "semantic");
        assert_eq!(links[0].alias, Some("related reading".to_string()));
    }

    #[test]
    fn test_mixed_typed_and_plain_wikilinks() {
        let links =
            extract_wikilinks("Plain [[Note A]] and typed [[rel:temporal|Note B]] together");
        assert_eq!(links.len(), 2);
        // Typed links come first
        assert_eq!(links[0].target, "Note B");
        assert_eq!(links[0].relation, "temporal");
        assert_eq!(links[1].target, "Note A");
        assert_eq!(links[1].relation, "wikilink");
    }
}

use std::collections::HashSet;

/// Returns true if the image's tags satisfy both the hotkey-tag set and the free-word filter.
///
/// - Every entry of `required_tags` must be present (case-sensitive) in `image_tags`.
/// - If `free_word` is non-empty, at least one of the image's tags must contain it
///   as a case-insensitive substring.
/// - Both filters empty ⇒ the image always matches.
pub fn matches(
    image_tags: &[String],
    required_tags: &HashSet<String>,
    free_word: &str,
) -> bool {
    if !required_tags
        .iter()
        .all(|req| image_tags.iter().any(|t| t == req))
    {
        return false;
    }

    let trimmed = free_word.trim();
    if trimmed.is_empty() {
        return true;
    }
    let needle = trimmed.to_lowercase();
    image_tags
        .iter()
        .any(|t| t.to_lowercase().contains(&needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_filters_match_everything() {
        assert!(matches(&tags(&["cat"]), &HashSet::new(), ""));
        assert!(matches(&[], &HashSet::new(), ""));
    }

    #[test]
    fn required_tags_must_all_be_present() {
        let img = tags(&["cat", "cute"]);
        assert!(matches(&img, &set(&["cat"]), ""));
        assert!(matches(&img, &set(&["cat", "cute"]), ""));
        assert!(!matches(&img, &set(&["cat", "dog"]), ""));
    }

    #[test]
    fn free_word_is_substring_case_insensitive() {
        let img = tags(&["Cat", "Outdoor"]);
        assert!(matches(&img, &HashSet::new(), "cat"));
        assert!(matches(&img, &HashSet::new(), "OUT"));
        assert!(!matches(&img, &HashSet::new(), "dog"));
    }

    #[test]
    fn free_word_and_tags_combine_with_and() {
        let img = tags(&["cat", "outdoor"]);
        assert!(matches(&img, &set(&["cat"]), "out"));
        assert!(!matches(&img, &set(&["dog"]), "out"));
        assert!(!matches(&img, &set(&["cat"]), "indoor"));
    }
}

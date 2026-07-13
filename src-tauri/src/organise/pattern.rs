//! Organisation pattern parsing and per-sample folder resolution.
//!
//! A pattern is a path template with `{Dimension}` placeholders, e.g.
//! `{Type}/{Instrument}`. Resolution turns a sample's primary tags into the
//! folder segments a file should live under. Samples missing a tag on any
//! used dimension fall back to the `_untagged` folder.

use crate::error::CommandError;
use std::collections::HashMap;

pub const UNTAGGED_FOLDER: &str = "_untagged";

#[derive(Debug, Clone, PartialEq)]
enum PatternPart {
    Literal(String),
    Placeholder(String),
}

/// A parsed pattern: one entry per folder level, each a mix of literal text
/// and dimension placeholders.
#[derive(Debug, Clone)]
pub struct OrganisePattern {
    segments: Vec<Vec<PatternPart>>,
    dimensions: Vec<String>,
}

impl OrganisePattern {
    pub fn parse(pattern: &str) -> Result<Self, CommandError> {
        let normalized = pattern.trim().replace('\\', "/");
        if normalized.is_empty() {
            return Err(CommandError::Other("Pattern is empty".to_string()));
        }

        let mut segments = Vec::new();
        let mut dimensions: Vec<String> = Vec::new();
        for raw_segment in normalized.split('/') {
            if raw_segment.is_empty() {
                continue;
            }
            let parts = parse_segment(raw_segment)?;
            for part in &parts {
                if let PatternPart::Placeholder(name) = part {
                    if !dimensions.contains(name) {
                        dimensions.push(name.clone());
                    }
                }
            }
            segments.push(parts);
        }

        if segments.is_empty() {
            return Err(CommandError::Other(
                "Pattern contains no folder segments".to_string(),
            ));
        }

        Ok(Self {
            segments,
            dimensions,
        })
    }

    /// Distinct dimension names referenced by the pattern, in order of first use.
    pub fn dimensions(&self) -> &[String] {
        &self.dimensions
    }

    /// Resolve the folder segments for one sample given its primary tag value
    /// per dimension. Returns `None` when any referenced dimension has no tag,
    /// in which case the caller uses the `_untagged` fallback.
    pub fn resolve(&self, tags: &HashMap<String, String>) -> Option<Vec<String>> {
        let mut folders = Vec::with_capacity(self.segments.len());
        for segment in &self.segments {
            let mut folder = String::new();
            for part in segment {
                match part {
                    PatternPart::Literal(text) => folder.push_str(text),
                    PatternPart::Placeholder(name) => folder.push_str(tags.get(name)?),
                }
            }
            let folder = sanitize_path_component(&folder);
            if !folder.is_empty() {
                folders.push(folder);
            }
        }
        Some(folders)
    }
}

fn parse_segment(segment: &str) -> Result<Vec<PatternPart>, CommandError> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = segment.chars();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if !literal.is_empty() {
                    parts.push(PatternPart::Literal(std::mem::take(&mut literal)));
                }
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') => {
                            return Err(CommandError::Other(
                                "Nested '{' in pattern placeholder".to_string(),
                            ))
                        }
                        Some(c) => name.push(c),
                        None => {
                            return Err(CommandError::Other(
                                "Unclosed '{' in pattern".to_string(),
                            ))
                        }
                    }
                }
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(CommandError::Other(
                        "Empty placeholder in pattern".to_string(),
                    ));
                }
                parts.push(PatternPart::Placeholder(name));
            }
            '}' => {
                return Err(CommandError::Other(
                    "Unmatched '}' in pattern".to_string(),
                ))
            }
            c => literal.push(c),
        }
    }

    if !literal.is_empty() {
        parts.push(PatternPart::Literal(literal));
    }
    Ok(parts)
}

/// Windows reserved device names that cannot be used as file or folder names.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Make a resolved segment safe as a single folder name on Windows and macOS:
/// path separators and other invalid characters become `-`, trailing dots and
/// surrounding whitespace are trimmed, and reserved device names are prefixed.
pub fn sanitize_path_component(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 0x20 => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();

    if trimmed.is_empty() {
        return String::new();
    }
    if RESERVED_NAMES
        .iter()
        .any(|name| trimmed.eq_ignore_ascii_case(name))
    {
        return format!("_{trimmed}");
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn parses_placeholders_in_order() {
        let pattern = OrganisePattern::parse("{Type}/{Instrument}").unwrap();
        assert_eq!(pattern.dimensions(), ["Type", "Instrument"]);
    }

    #[test]
    fn duplicate_placeholders_are_reported_once() {
        let pattern = OrganisePattern::parse("{Type}/{Type} {Instrument}").unwrap();
        assert_eq!(pattern.dimensions(), ["Type", "Instrument"]);
    }

    #[test]
    fn resolves_simple_pattern() {
        let pattern = OrganisePattern::parse("{Type}/{Instrument}").unwrap();
        let folders = pattern
            .resolve(&tags(&[("Type", "loop"), ("Instrument", "kick")]))
            .unwrap();
        assert_eq!(folders, ["loop", "kick"]);
    }

    #[test]
    fn resolves_mixed_literal_and_placeholder_segments() {
        let pattern = OrganisePattern::parse("Sorted/{Type}-{Key}").unwrap();
        let folders = pattern
            .resolve(&tags(&[("Type", "loop"), ("Key", "C#")]))
            .unwrap();
        assert_eq!(folders, ["Sorted", "loop-C#"]);
    }

    #[test]
    fn missing_tag_returns_none() {
        let pattern = OrganisePattern::parse("{Type}/{Instrument}").unwrap();
        assert!(pattern.resolve(&tags(&[("Type", "loop")])).is_none());
    }

    #[test]
    fn empty_segments_are_dropped() {
        let pattern = OrganisePattern::parse("{Type}//{Instrument}/").unwrap();
        let folders = pattern
            .resolve(&tags(&[("Type", "loop"), ("Instrument", "kick")]))
            .unwrap();
        assert_eq!(folders, ["loop", "kick"]);
    }

    #[test]
    fn backslash_separators_are_accepted() {
        let pattern = OrganisePattern::parse("{Type}\\{Instrument}").unwrap();
        assert_eq!(pattern.dimensions(), ["Type", "Instrument"]);
    }

    #[test]
    fn rejects_invalid_patterns() {
        assert!(OrganisePattern::parse("").is_err());
        assert!(OrganisePattern::parse("   ").is_err());
        assert!(OrganisePattern::parse("{Type").is_err());
        assert!(OrganisePattern::parse("Type}").is_err());
        assert!(OrganisePattern::parse("{}").is_err());
        assert!(OrganisePattern::parse("{Ty{pe}}").is_err());
        assert!(OrganisePattern::parse("/").is_err());
    }

    #[test]
    fn sanitizes_invalid_filename_characters() {
        assert_eq!(sanitize_path_component("a/b:c*d"), "a-b-c-d");
        assert_eq!(sanitize_path_component("  loop  "), "loop");
        assert_eq!(sanitize_path_component("name..."), "name");
        assert_eq!(sanitize_path_component("..."), "");
        assert_eq!(sanitize_path_component(".."), "");
        assert_eq!(sanitize_path_component("C#"), "C#");
    }

    #[test]
    fn sanitizes_reserved_windows_names() {
        assert_eq!(sanitize_path_component("aux"), "_aux");
        assert_eq!(sanitize_path_component("COM1"), "_COM1");
        assert_eq!(sanitize_path_component("auxiliary"), "auxiliary");
    }

    #[test]
    fn tag_value_with_separator_stays_one_folder_level() {
        let pattern = OrganisePattern::parse("{Mood}").unwrap();
        let folders = pattern.resolve(&tags(&[("Mood", "dark/moody")])).unwrap();
        assert_eq!(folders, ["dark-moody"]);
    }
}

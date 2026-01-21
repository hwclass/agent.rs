//! Skill manifest parsing (Agent Skills spec frontmatter)
//!
//! This module parses SKILL.md frontmatter (YAML) into a typed struct so hosts
//! can implement progressive disclosure and discovery.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parsed frontmatter of a SKILL.md file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

/// Full manifest with body content
#[derive(Debug, Clone, PartialEq)]
pub struct SkillManifest {
    pub frontmatter: SkillFrontmatter,
    /// The body of SKILL.md after the frontmatter (left raw for host usage)
    pub body: String,
}

/// Errors while parsing a skill manifest
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SkillManifestError {
    #[error("missing frontmatter delimiter '---'")]
    MissingDelimiter,
    #[error("frontmatter not found")]
    MissingFrontmatter,
    #[error("failed to parse frontmatter: {0}")]
    FrontmatterParse(String),
}

/// Parse a SKILL.md string into a manifest (frontmatter + body).
/// Expects YAML frontmatter delimited by `---` at the start of the file.
pub fn parse_skill_manifest(markdown: &str) -> Result<SkillManifest, SkillManifestError> {
    let mut lines = markdown.lines();
    let first = lines.next().ok_or(SkillManifestError::MissingFrontmatter)?;

    if first.trim() != "---" {
        return Err(SkillManifestError::MissingDelimiter);
    }

    let mut frontmatter_raw = String::new();
    let mut in_frontmatter = true;

    for line in lines.by_ref() {
        if line.trim() == "---" {
            in_frontmatter = false;
            break;
        }
        frontmatter_raw.push_str(line);
        frontmatter_raw.push('\n');
    }

    if in_frontmatter {
        return Err(SkillManifestError::MissingFrontmatter);
    }

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(&frontmatter_raw)
        .map_err(|e| SkillManifestError::FrontmatterParse(e.to_string()))?;

    let body = lines.collect::<Vec<_>>().join("\n");

    Ok(SkillManifest { frontmatter, body })
}

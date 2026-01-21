use agent_core::skill_manifest::{parse_skill_manifest, SkillManifest, SkillManifestError};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    pub path: PathBuf,
    pub manifest: SkillManifest,
}

/// Discover skills by scanning provided directories for SKILL.md files.
pub fn discover_skills(skill_dirs: &[PathBuf]) -> Vec<DiscoveredSkill> {
    let mut found = Vec::new();

    for dir in skill_dirs {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("SKILL.md");
                    if manifest_path.exists() {
                        if let Some(skill) = load_skill_manifest(&manifest_path) {
                            found.push(skill);
                        }
                    }
                }
            }
        }
    }

    found
}

fn load_skill_manifest(path: &Path) -> Option<DiscoveredSkill> {
    let content = fs::read_to_string(path).ok()?;
    match parse_skill_manifest(&content) {
        Ok(manifest) => Some(DiscoveredSkill {
            path: path.to_path_buf(),
            manifest,
        }),
        Err(err) => {
            eprintln!(
                "⚠️  Failed to parse skill manifest {}: {}",
                path.display(),
                format_manifest_error(err)
            );
            None
        }
    }
}

fn format_manifest_error(err: SkillManifestError) -> String {
    match err {
        SkillManifestError::MissingDelimiter => "missing YAML frontmatter delimiter".to_string(),
        SkillManifestError::MissingFrontmatter => "missing YAML frontmatter content".to_string(),
        SkillManifestError::FrontmatterParse(msg) => format!("invalid frontmatter: {}", msg),
    }
}

/// Build an XML block compatible with Agent Skills prompt format.
pub fn build_available_skills_prompt(skills: &[DiscoveredSkill]) -> String {
    let mut out = String::from("<available_skills>\n");

    for skill in skills {
        out.push_str("<skill>\n");
        out.push_str("<name>\n");
        out.push_str(&skill.manifest.frontmatter.name);
        out.push_str("\n</name>\n");
        out.push_str("<description>\n");
        out.push_str(&skill.manifest.frontmatter.description);
        out.push_str("\n</description>\n");
        out.push_str("<location>\n");
        out.push_str(&skill.path.to_string_lossy());
        out.push_str("\n</location>\n");
        out.push_str("</skill>\n");
    }

    out.push_str("</available_skills>");
    out
}

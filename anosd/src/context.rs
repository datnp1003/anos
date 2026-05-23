use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub system_prompt: String,
    pub skills: HashMap<String, Skill>,
}

impl PromptContext {
    pub fn load(dir: &str) -> Result<Self> {
        let sp = Self::load_system_prompt(dir)?;
        let skills = Self::load_skills(dir)?;
        tracing::info!(
            "System prompt ({} chars), {} skills",
            sp.len(),
            skills.len()
        );
        Ok(Self {
            system_prompt: sp,
            skills,
        })
    }

    fn load_system_prompt(dir: &str) -> Result<String> {
        let content = std::fs::read_to_string(format!("{}/ANOS-SYSTEM-PROMPT.md", dir))?;
        let body = if content.starts_with("---") {
            content[4..]
                .find("---")
                .map(|e| content[4 + e + 4..].trim().to_string())
                .unwrap_or(content)
        } else {
            content
        };
        tracing::info!("Loaded system prompt ({} chars)", body.len());
        Ok(body)
    }

    fn load_skills(dir: &str) -> Result<HashMap<String, Skill>> {
        let skills_dir = format!("{}/skills", dir);
        let path = std::path::Path::new(&skills_dir);
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let mut skills = HashMap::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            let skill_file = entry.path().join("SKILL.md");
            if !skill_file.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&skill_file)?;
            let (name, desc, body) = if content.starts_with("---") {
                let end = content[4..].find("---").unwrap_or(0);
                let fm = &content[4..end + 4];
                let body = content[end + 8..].trim().to_string();
                (
                    extract_yaml(fm, "name"),
                    extract_yaml(fm, "description"),
                    body,
                )
            } else {
                (
                    entry.file_name().to_string_lossy().to_string(),
                    String::new(),
                    content,
                )
            };
            skills.insert(
                name.clone(),
                Skill {
                    name,
                    description: desc,
                    body,
                },
            );
        }
        tracing::info!("Loaded {} skills", skills.len());
        Ok(skills)
    }

    pub fn find_skill(&self, hint: &str) -> Option<&Skill> {
        self.skills
            .iter()
            .find(|(n, s)| {
                n.contains(hint) || s.description.contains(hint) || s.body.contains(hint)
            })
            .map(|(_, s)| s)
    }

    pub fn build_system_prompt(
        &self,
        skill_name: Option<&str>,
        system_map: Option<&str>,
        intent_hint: Option<&str>,
    ) -> String {
        let mut prompt = self.system_prompt.clone();
        if let Some(map) = system_map {
            prompt.push_str("\n\n---\n\n");
            prompt.push_str(map);
        }
        if let Some(name) = skill_name {
            if let Some(s) = self.find_skill(name) {
                prompt.push_str(&format!(
                    "\n\n---\n\n## Active Skill: {}\n\n{}",
                    s.name, s.body
                ));
            }
        }
        if skill_name.is_none() {
            if let Some(hint) = intent_hint {
                if let Some(s) = self.find_skill(hint) {
                    prompt.push_str(&format!(
                        "\n\n---\n\n## Related Skill: {}\n\n{}",
                        s.name, s.body
                    ));
                }
            }
        }
        if !self.skills.is_empty() {
            prompt.push_str("\n\n---\n\n## Skills Available\n");
            for s in self.skills.values() {
                prompt.push_str(&format!("- **{}**: {}\n", s.name, s.description));
            }
        }
        prompt
    }
}

fn extract_yaml(fm: &str, key: &str) -> String {
    for line in fm.lines() {
        if let Some(v) = line.trim().strip_prefix(&format!("{}:", key)) {
            return v.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }
    String::new()
}

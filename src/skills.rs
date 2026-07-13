use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub init_code: String,
    pub commands: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct JsonSkill {
    name: Option<String>,
    description: Option<String>,
    prompt: Option<String>,
    init: Option<String>,
    commands: Option<std::collections::HashMap<String, String>>,
}

impl Skill {
    pub fn load_dirs(dirs: &[PathBuf]) -> Vec<Skill> {
        let mut skills = Vec::new();
        for dir in dirs {
            if dir.is_dir() {
                skills.extend(Self::load_dir(dir));
            }
        }
        skills
    }

    fn load_dir(dir: &Path) -> Vec<Skill> {
        let mut skills = Vec::new();
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return skills,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let skill = match path.extension().and_then(|e| e.to_str()) {
                Some("alisp") => Self::load_alisp(&path),
                Some("json") => Self::load_json(&path),
                _ => continue,
            };

            if let Some(s) = skill {
                skills.push(s);
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }

    fn load_alisp(path: &Path) -> Option<Skill> {
        let content = fs::read_to_string(path).ok()?;
        let mut name = path.file_stem()?.to_str()?.to_string();
        let mut description = String::new();
        let mut prompt = String::new();
        let mut code_start = 0;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("; name:") {
                name = val.trim().to_string();
                code_start = i + 1;
            } else if let Some(val) = trimmed.strip_prefix("; description:") {
                description = val.trim().to_string();
                code_start = i + 1;
            } else if let Some(val) = trimmed.strip_prefix("; prompt:") {
                prompt = val.trim().to_string();
                code_start = i + 1;
            } else if trimmed.starts_with(";") && !trimmed.starts_with("; ") && trimmed.len() > 1 {
                break;
            } else if !trimmed.starts_with(";") && !trimmed.is_empty() {
                break;
            } else {
                code_start = i + 1;
            }
        }

        let init_code = content.lines().skip(code_start).collect::<Vec<_>>().join("\n");

        Some(Skill {
            name,
            description,
            prompt,
            init_code,
            commands: Vec::new(),
        })
    }

    fn load_json(path: &Path) -> Option<Skill> {
        let content = fs::read_to_string(path).ok()?;
        let json: JsonSkill = serde_json::from_str(&content).ok()?;

        let name = json.name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let commands: Vec<(String, String)> = json
            .commands
            .map(|m| m.into_iter().collect())
            .unwrap_or_default();

        let mut init_code = String::new();
        if let Some(init) = &json.init {
            init_code.push_str(init);
        }
        for (cmd_name, cmd_code) in &commands {
            init_code.push_str(&format!(
                "\n(defn {} () {})",
                cmd_name, cmd_code
            ));
        }

        Some(Skill {
            name,
            description: json.description.unwrap_or_default(),
            prompt: json.prompt.unwrap_or_default(),
            init_code,
            commands,
        })
    }

    pub fn skill_index(skills: &[Skill]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut out = String::from("\n## Available Skills\n\n");
        for skill in skills {
            out.push_str(&format!("### {}\n", skill.name));
            if !skill.description.is_empty() {
                out.push_str(&format!("{}\n", skill.description));
            }
            if !skill.commands.is_empty() {
                out.push_str("Commands:\n");
                for (name, _) in &skill.commands {
                    out.push_str(&format!("  - `({})` — see skill docs\n", name));
                }
            }
            out.push('\n');
        }
        out
    }
}

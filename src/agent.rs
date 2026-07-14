use crate::config::AgentConfig;
use crate::llm::{LlmBackend, Message, Role};
use crate::memory::MemoryManager;
use crate::security::{SecurityConfig, SecurityPolicy};
use crate::skills::Skill;
use crate::tools::AlispHost;
use std::collections::HashSet;

const SYSTEM_PROMPT: &str = include_str!("../prompt.md");

/// Rough token estimation: ~4 chars per token for English text.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

pub struct Agent {
    messages: Vec<Message>,
    tools: AlispHost,
    policy: SecurityPolicy,
    max_turns: u32,
    max_context_tokens: usize,
    loaded_skills: HashSet<String>,
}

impl Agent {
    pub fn new(
        config: AgentConfig,
        security: SecurityConfig,
        skills: &[Skill],
        memory: &MemoryManager,
    ) -> Self {
        let policy = SecurityPolicy::new(security.clone());
        let mut tools = AlispHost::with_policy(policy.clone());

        let mut system_prompt = SYSTEM_PROMPT.to_string();
        let mut loaded_skills = HashSet::new();

        // Initialize memory database
        let mem_init = memory.init_code();
        if let Err(e) = tools.execute(&mem_init) {
            eprintln!("warning: memory init failed: {}", e);
        }

        for skill in skills {
            loaded_skills.insert(skill.name.clone());
            if !skill.prompt.is_empty() {
                system_prompt.push_str(&format!("\n\n{}", skill.prompt));
            }
            if !skill.init_code.is_empty() {
                if let Err(e) = tools.execute(&skill.init_code) {
                    eprintln!("warning: skill '{}' init failed: {}", skill.name, e);
                }
            }
        }

        system_prompt.push_str(&Skill::skill_index(skills));

        Self {
            messages: vec![Message {
                role: Role::System,
                content: system_prompt,
            }],
            tools,
            policy,
            max_turns: config.max_turns,
            max_context_tokens: config.max_context_tokens,
            loaded_skills,
        }
    }

    /// Refresh skills: initialize any new skills and update the system prompt.
    pub fn refresh_skills(&mut self, skills: &[Skill]) {
        let mut new_count = 0;
        let mut new_skill_text = String::new();

        for skill in skills {
            if self.loaded_skills.contains(&skill.name) {
                continue;
            }
            self.loaded_skills.insert(skill.name.clone());
            new_count += 1;

            eprintln!("hotreload: loaded skill '{}'", skill.name);

            if !skill.prompt.is_empty() {
                new_skill_text.push_str(&format!("\n\n{}", skill.prompt));
            }
            if !skill.init_code.is_empty() {
                if let Err(e) = self.tools.execute(&skill.init_code) {
                    eprintln!("warning: skill '{}' init failed: {}", skill.name, e);
                }
            }
        }

        if new_count > 0 {
            // Rebuild skill index with all skills
            let index = Skill::skill_index(skills);

            // Find and replace the old skill index in the system prompt
            let sys_msg = &mut self.messages[0].content;
            if let Some(pos) = sys_msg.find("\n## Available Skills") {
                sys_msg.truncate(pos);
            }
            sys_msg.push_str(&new_skill_text);
            sys_msg.push_str(&index);

            eprintln!(
                "hotreload: {} new skill(s) available (total: {})",
                new_count,
                self.loaded_skills.len()
            );
        }
    }

    fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| estimate_tokens(&m.content)).sum()
    }

    fn truncate_context(&mut self) {
        while self.total_tokens() > self.max_context_tokens && self.messages.len() > 2 {
            let second = &self.messages[1];
            if second.role == Role::User {
                let removed = self.messages.remove(1);
                let removed_tokens = estimate_tokens(&removed.content);

                self.messages.insert(
                    1,
                    Message {
                        role: Role::User,
                        content: format!(
                            "[Earlier message truncated ({} tokens)]",
                            removed_tokens
                        ),
                    },
                );
            } else {
                break;
            }
        }

        if self.total_tokens() > self.max_context_tokens && self.messages.len() > 3 {
            let removed = self.messages.remove(1);
            let removed_tokens = estimate_tokens(&removed.content);
            self.messages.insert(
                1,
                Message {
                    role: Role::User,
                    content: format!(
                        "[Earlier messages truncated ({} tokens)]",
                        removed_tokens
                    ),
                },
            );
        }
    }

    #[allow(dead_code)]
    pub fn run(&mut self, backend: &mut dyn LlmBackend, user_input: &str) -> Result<String, String> {
        self.messages.push(Message {
            role: Role::User,
            content: user_input.to_string(),
        });

        self.truncate_context();

        self.run_loop(backend, None)
    }

    pub fn run_streaming(
        &mut self,
        backend: &mut dyn LlmBackend,
        user_input: &str,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        self.messages.push(Message {
            role: Role::User,
            content: user_input.to_string(),
        });

        self.truncate_context();

        self.run_loop(backend, Some(on_token))
    }

    fn run_loop(
        &mut self,
        backend: &mut dyn LlmBackend,
        mut on_token: Option<&mut dyn FnMut(&str)>,
    ) -> Result<String, String> {
        for _ in 0..self.max_turns {
            self.policy.start_turn();

            let response = if let Some(ref mut callback) = on_token {
                backend.complete_streaming(&self.messages, callback)?
            } else {
                backend.complete(&self.messages)?
            };

            if response.trim().is_empty() {
                return Ok(String::new());
            }

            let blocks = extract_alisp_blocks(&response);

            if blocks.is_empty() {
                self.messages.push(Message {
                    role: Role::Assistant,
                    content: response.clone(),
                });
                return Ok(response);
            }

            self.messages.push(Message {
                role: Role::Assistant,
                content: response.clone(),
            });

            let mut tool_output = String::new();
            for code in &blocks {
                let result = self.tools.execute(code);
                let output = match result {
                    Ok(val) => val,
                    Err(e) => format!("error: {}", e),
                };
                let output = self.policy.check_output(&output);
                tool_output.push_str(&format!("```\n{}\n```\n", output));
            }

            self.messages.push(Message {
                role: Role::Tool,
                content: tool_output,
            });
        }

        Err("max turns exceeded".to_string())
    }

    #[allow(dead_code)]
    pub fn clear_history(&mut self) {
        self.messages.truncate(1);
    }
}

fn extract_alisp_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("```alisp") {
        let after_tag = start + 8;
        if let Some(end) = remaining[after_tag..].find("```") {
            let code = remaining[after_tag..after_tag + end].trim().to_string();
            if !code.is_empty() {
                blocks.push(code);
            }
            remaining = &remaining[after_tag + end + 3..];
        } else {
            break;
        }
    }

    blocks
}

pub mod stdin;
pub mod llamacpp;
pub mod openai;

use std::fmt;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

pub trait LlmBackend {
    fn complete(&mut self, messages: &[Message]) -> Result<String, String>;

    fn complete_streaming(
        &mut self,
        messages: &[Message],
        _on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        self.complete(messages)
    }
}

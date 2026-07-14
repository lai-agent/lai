pub mod llamacpp;
pub mod openai;
pub mod stdin;

use std::fmt;
use std::io::{BufRead, BufReader};

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

/// Parse an SSE (Server-Sent Events) stream from a reader, extracting content tokens.
/// Calls `on_token` for each token and returns the full accumulated response.
pub fn parse_sse_stream(
    reader: impl std::io::Read,
    on_token: &mut dyn FnMut(&str),
) -> Result<String, String> {
    let buf_reader = BufReader::new(reader);
    let mut full_response = String::new();

    for line in buf_reader.lines() {
        let line = line.map_err(|e| format!("read stream: {}", e))?;
        let line = line.trim();

        if line.is_empty() || !line.starts_with("data: ") {
            continue;
        }

        let data = &line[6..];
        if data == "[DONE]" {
            break;
        }

        if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                if let Some(choice) = choices.first() {
                    if let Some(delta) = choice.get("delta") {
                        let token = delta
                            .get("content")
                            .and_then(|c| c.as_str())
                            .or_else(|| delta.get("reasoning").and_then(|r| r.as_str()));
                        if let Some(token) = token {
                            full_response.push_str(token);
                            on_token(token);
                        }
                    }
                }
            }
        }
    }

    Ok(full_response)
}

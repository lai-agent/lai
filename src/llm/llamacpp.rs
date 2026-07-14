use super::{parse_sse_stream, LlmBackend, Message, Role};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct LlamaCppBackend {
    url: String,
    model: String,
    agent: ureq::Agent,
    temperature: f64,
    max_tokens: u32,
}

fn make_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(300)))
            .build(),
    )
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

impl LlamaCppBackend {
    #[allow(dead_code)]
    pub fn new(url: &str, model: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            agent: make_agent(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }

    pub fn with_params(url: &str, model: &str, temperature: f64, max_tokens: u32) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            agent: make_agent(),
            temperature,
            max_tokens,
        }
    }

    fn map_messages(messages: &[Message]) -> Vec<ChatMessage> {
        messages
            .iter()
            .map(|m| {
                let (role, content) = match m.role {
                    Role::System => ("system".to_string(), m.content.clone()),
                    Role::User => ("user".to_string(), m.content.clone()),
                    Role::Assistant => ("assistant".to_string(), m.content.clone()),
                    Role::Tool => (
                        "user".to_string(),
                        format!("[Tool Result]\n{}", m.content),
                    ),
                };
                ChatMessage { role, content }
            })
            .collect()
    }
}

impl LlmBackend for LlamaCppBackend {
    fn complete(&mut self, messages: &[Message]) -> Result<String, String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: Self::map_messages(messages),
            temperature: Some(self.temperature),
            max_tokens: Some(self.max_tokens),
            stream: false,
        };

        let url = format!("{}/chat/completions", self.url);
        let mut resp = self
            .agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send_json(&request)
            .map_err(|e| format!("request failed: {}", e))?;

        let body: ChatResponse = resp
            .body_mut()
            .read_json()
            .map_err(|e| format!("read body: {}", e))?;

        body.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| "empty response".to_string())
    }

    fn complete_streaming(
        &mut self,
        messages: &[Message],
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: Self::map_messages(messages),
            temperature: Some(self.temperature),
            max_tokens: Some(self.max_tokens),
            stream: true,
        };

        let url = format!("{}/chat/completions", self.url);
        let mut resp = self
            .agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send_json(&request)
            .map_err(|e| format!("request failed: {}", e))?;

        parse_sse_stream(resp.body_mut().as_reader(), on_token)
    }
}

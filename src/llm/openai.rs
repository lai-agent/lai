use super::{parse_sse_stream, LlmBackend, Message, Role};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct OpenAIBackend {
    url: String,
    model: String,
    api_key: String,
    agent: ureq::Agent,
    temperature: f64,
    max_tokens: u32,
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

#[derive(Serialize, Deserialize, Clone)]
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
    reasoning: Option<String>,
}

fn make_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(300)))
            .build(),
    )
}

impl OpenAIBackend {
    #[allow(dead_code)]
    pub fn new(url: &str, model: &str, api_key: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            agent: make_agent(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }

    pub fn with_params(
        url: &str,
        model: &str,
        api_key: &str,
        temperature: f64,
        max_tokens: u32,
    ) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            agent: make_agent(),
            temperature,
            max_tokens,
        }
    }

    fn map_messages(messages: &[Message]) -> Vec<ChatMessage> {
        messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "user",
                };
                let content = if m.role == Role::Tool {
                    format!("[Tool Result]\n{}", m.content)
                } else {
                    m.content.clone()
                };
                ChatMessage {
                    role: role.to_string(),
                    content,
                }
            })
            .collect()
    }

    fn send_request(
        &self,
        messages: &[ChatMessage],
        stream: bool,
    ) -> Result<ureq::Body, String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: Some(self.temperature),
            max_tokens: Some(self.max_tokens),
            stream,
        };

        let url = format!("{}/chat/completions", self.url);
        let resp = self
            .agent
            .post(&url)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.api_key),
            )
            .send_json(&request)
            .map_err(|e| format!("request failed: {}", e))?;

        Ok(resp.into_body())
    }
}

impl LlmBackend for OpenAIBackend {
    fn complete(&mut self, messages: &[Message]) -> Result<String, String> {
        let chat_messages = Self::map_messages(messages);
        let mut body = self.send_request(&chat_messages, false)?;

        let resp: ChatResponse = body
            .read_json()
            .map_err(|e| format!("read body: {}", e))?;

        resp.choices
            .first()
            .and_then(|c| c.message.content.clone().or(c.message.reasoning.clone()))
            .ok_or_else(|| "empty response".to_string())
    }

    fn complete_streaming(
        &mut self,
        messages: &[Message],
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let chat_messages = Self::map_messages(messages);
        let mut body = self.send_request(&chat_messages, true)?;

        parse_sse_stream(body.as_reader(), on_token)
    }
}

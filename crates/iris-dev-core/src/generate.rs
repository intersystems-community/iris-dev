use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct LlmClient {
    model: String,
    api_key: String,
    timeout_secs: u64,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
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
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

impl LlmClient {
    pub fn from_env() -> Option<Self> {
        let model = std::env::var("IRIS_GENERATE_CLASS_MODEL").ok()?;
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .ok()?;
        let timeout_secs = std::env::var("IRIS_GENERATE_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        Some(Self { model, api_key, timeout_secs })
    }

    pub async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build()?;

        let base_url = if self.model.starts_with("claude") {
            "https://api.anthropic.com/v1/messages"
        } else {
            "https://api.openai.com/v1/chat/completions"
        };

        let resp = client
            .post(base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&ChatRequest {
                model: self.model.clone(),
                messages: vec![
                    ChatMessage { role: "system".to_string(), content: system.to_string() },
                    ChatMessage { role: "user".to_string(), content: user.to_string() },
                ],
            })
            .send().await
            .context("LLM API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error {}: {}", status, body);
        }

        let parsed: ChatResponse = resp.json().await.context("parsing LLM response")?;
        parsed.choices.into_iter().next()
            .map(|c| c.message.content)
            .context("empty LLM response")
    }
}

pub const GENERATE_CLASS_SYSTEM: &str = r#"You are an InterSystems ObjectScript expert. Generate a complete, compilable ObjectScript class in UDL format.

Rules:
- Start with: Class <ClassName> Extends <Superclass>
- Use { } for method bodies, NOT begin/end
- All methods must have a closing }
- The class block must end with a single }
- Return ONLY the class definition — no explanations, no markdown fences"#;

pub const GENERATE_TEST_SYSTEM: &str = r#"You are an InterSystems ObjectScript testing expert. Generate a complete %UnitTest.TestCase subclass in UDL format.

Rules:
- Extend %UnitTest.TestCase
- Test methods MUST start with "Test" prefix
- Use $$$AssertEquals, $$$AssertTrue, $$$AssertNotNull macros
- Return ONLY the test class definition — no explanations, no markdown fences"#;

pub const RETRY_TEMPLATE: &str = "The generated class failed to compile with these errors:\n\n{errors}\n\nPlease fix the ObjectScript class. Return ONLY the corrected class definition.";

pub fn validate_cls_syntax(text: &str) -> bool {
    text.contains("Class ") && text.contains('{') && text.matches('{').count() == text.matches('}').count()
}

pub fn extract_class_name(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.trim_start().starts_with("Class "))
        .and_then(|l| l.split_whitespace().nth(1))
        .map(|s| s.to_string())
}

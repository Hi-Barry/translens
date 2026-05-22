use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
type StreamResult = Result<Bytes, reqwest::Error>;

/// DeepSeek streaming translator
/// Uses OpenAI-compatible chat completions API with SSE streaming

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    temperature: f32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChunkResponse {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

pub struct DeepSeekTranslator {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    temperature: f32,
}

impl DeepSeekTranslator {
    pub fn new(
        api_key: &str,
        base_url: &str,
        model: &str,
        temperature: f32,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: api_key.to_string(),
            base_url: if base_url.is_empty() {
                "https://api.deepseek.com/v1".to_string()
            } else {
                base_url.trim_end_matches('/').to_string()
            },
            model: if model.is_empty() {
                "deepseek-chat".to_string()
            } else {
                model.to_string()
            },
            temperature,
        }
    }

    /// Translate text with streaming response
    pub async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        // Build the system prompt based on languages
        let target_name = match target_lang {
            "zh-CN" | "zh" => "简体中文",
            "en" => "English",
            "ja" => "Japanese",
            "ko" => "Korean",
            "fr" => "French",
            "de" => "German",
            _ => target_lang,
        };

        let system_prompt = format!(
            "You are a professional translator. Translate the following text to {}. \
             Return ONLY the translation, no explanations, no notes, no quotes. \
             Preserve any formatting like line breaks.",
            target_name
        );

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            stream: true,
            temperature: self.temperature,
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, body).into());
        }

        // Parse SSE stream
        let stream = response.bytes_stream();

        let mapped = stream.map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    let mut output = String::new();

                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }
                            if let Ok(chunk_resp) =
                                serde_json::from_str::<ChunkResponse>(data)
                            {
                                for choice in chunk_resp.choices {
                                    if let Some(content) = choice.delta.content {
                                        output.push_str(&content);
                                    }
                                }
                            }
                        }
                    }

                    // Skip empty chunks (SSE heartbeats, non-content lines)
                    if output.is_empty() {
                        // Returning None is not possible with this stream type,
                        // so return an empty string that downstream filters out.
                        Ok(String::new())
                    } else {
                        Ok(output)
                    }
                }
                Err(e) => Err(format!("Stream error: {}", e)),
            }
        });

        Ok(Box::pin(mapped))
    }
}

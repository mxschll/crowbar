use anyhow::Result;
use futures_util::{Stream, StreamExt};
use reqwest::{header, Client};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub enum Provider {
    Anthropic,
    Ollama,
    OpenAI,
}

pub struct Copilot {
    provider: Provider,
    base_url: String,
    model: String,
    client: Client,
}

impl Copilot {
    pub fn new(
        provider: Provider,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self> {
        let api_key = api_key.into();
        let model = model.into();

        let base_url = match provider {
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::Anthropic => "https://api.anthropic.com/v1",
            Provider::Ollama => "http://localhost:11434/api",
        }
        .to_string();

        let headers = create_headers(&provider, &api_key)?;
        let client = Client::builder().default_headers(headers).build()?;

        Ok(Self {
            base_url,
            model,
            provider,
            client,
        })
    }

    pub async fn stream_chat<'a>(
        &'a self,
        prompt: &str,
    ) -> Result<impl Stream<Item = Result<String>> + 'a> {
        let endpoint = format!("{}{}", self.base_url, self.endpoint_suffix());
        let body = self.create_request_body(prompt);

        let response = self.client.post(&endpoint).json(&body).send().await?;
        if !response.status().is_success() {
            let error_body = response.text().await?;
            return Err(anyhow::anyhow!("Request failed: {}", error_body));
        }

        Ok(response.bytes_stream().map(move |chunk| {
            chunk
                .map_err(anyhow::Error::from)
                .and_then(|bytes| String::from_utf8(bytes.to_vec()).map_err(anyhow::Error::from))
                .and_then(|text| self.parse_stream_chunk(&text))
        }))
    }

    fn create_request_body(&self, prompt: &str) -> Value {
        match self.provider {
            Provider::OpenAI => json!({
                "model": self.model,
                "messages": [{ "role": "user", "content": prompt }],
                "max_tokens": 256,
                "stream": true
            }),
            Provider::Anthropic => json!({
                "model": self.model,
                "messages": [{ "role": "user", "content": prompt }],
                "max_tokens": 256,
                "stream": true,
                "system": "You are a helpful AI assistant."
            }),
            Provider::Ollama => json!({
                "model": self.model,
                "prompt": prompt,
                "stream": true
            }),
        }
    }

    fn parse_stream_chunk(&self, text: &str) -> Result<String> {
        match self.provider {
            Provider::OpenAI => parse_openai_chunk(text),
            Provider::Anthropic => parse_anthropic_chunk(text),
            Provider::Ollama => parse_ollama_chunk(text),
        }
    }

    fn endpoint_suffix(&self) -> &str {
        match self.provider {
            Provider::OpenAI => "/chat/completions",
            Provider::Anthropic => "/messages",
            Provider::Ollama => "/generate",
        }
    }
}

fn create_headers(provider: &Provider, api_key: &str) -> Result<header::HeaderMap> {
    let mut headers = header::HeaderMap::new();
    match provider {
        Provider::OpenAI => {
            headers.insert("Authorization", format!("Bearer {}", api_key).parse()?);
        }
        Provider::Anthropic => {
            headers.insert("x-api-key", api_key.parse()?);
            headers.insert("anthropic-version", "2023-06-01".parse()?);
        }
        Provider::Ollama => {}
    }
    Ok(headers)
}

fn parse_openai_chunk(text: &str) -> Result<String> {
    if !text.starts_with("data: ") {
        return Ok(String::new());
    }
    let data = text.trim_start_matches("data: ");
    if data == "[DONE]" {
        return Ok(String::new());
    }
    serde_json::from_str::<Value>(data)
        .map(|json| {
            json["choices"][0]["delta"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string()
        })
        .map_err(anyhow::Error::from)
}

fn parse_anthropic_chunk(text: &str) -> Result<String> {
    if text.is_empty() {
        return Ok(String::new());
    }
    text.split("\n\n")
        .filter(|s| !s.is_empty())
        .find_map(|message| {
            message
                .lines()
                .find(|line| line.starts_with("data: "))
                .and_then(|data_line| {
                    let json_str = data_line.trim_start_matches("data: ");
                    serde_json::from_str::<Value>(json_str).ok()
                })
                .and_then(|json| {
                    if json["type"] == "content_block_delta" {
                        json["delta"]["text"].as_str().map(String::from)
                    } else if json["type"] == "message_stop" {
                        Some(String::new())
                    } else {
                        None
                    }
                })
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to parse Anthropic chunk"))
}

fn parse_ollama_chunk(text: &str) -> Result<String> {
    serde_json::from_str::<Value>(text)
        .map(|json| json["response"].as_str().unwrap_or("").to_string())
        .map_err(anyhow::Error::from)
}

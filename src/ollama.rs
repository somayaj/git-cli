use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    /// Disable reasoning traces on thinking models (e.g. Qwen3) so output lands in `content`.
    think: bool,
    options: GenerateOptions,
    keep_alive: String,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct GenerateOptions {
    num_predict: u32,
    temperature: f32,
    stop: Vec<String>,
}

#[derive(Deserialize)]
struct ChatChunk {
    message: Option<ChunkMessage>,
    done: bool,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, Value>,
}

#[derive(Deserialize)]
struct ChunkMessage {
    content: String,
}

pub async fn generate(
    endpoint: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    keep_alive: &str,
) -> Result<String, String> {
    let url = format!("{endpoint}/api/chat");
    let client = Client::new();

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
        stream: true,
        think: false,
        options: GenerateOptions {
            num_predict: 512,
            temperature: 0.1,
            stop: vec!["\n\n\n".to_string()],
        },
        keep_alive: keep_alive.to_string(),
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama returned {status}: {body}"));
    }

    let mut full_response = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<ChatChunk>(line) {
                Ok(chunk) => {
                    if let Some(msg) = &chunk.message {
                        eprint!("{}", msg.content);
                        full_response.push_str(&msg.content);
                    }
                    if chunk.done {
                        eprintln!();
                        return Ok(full_response);
                    }
                }
                Err(_) => continue,
            }
        }
    }

    Ok(full_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_disables_thinking() {
        let req = ChatRequest {
            model: "qwen3:4b".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            stream: true,
            think: false,
            options: GenerateOptions {
                num_predict: 512,
                temperature: 0.1,
                stop: vec![],
            },
            keep_alive: "10m".to_string(),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json.get("think"), Some(&serde_json::Value::Bool(false)));
    }
}

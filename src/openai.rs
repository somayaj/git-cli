use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    max_tokens: u32,
    temperature: f32,
    stop: Vec<String>,
    /// Disable Qwen3 reasoning on llama-server so output lands in `content`.
    chat_template_kwargs: ChatTemplateKwargs,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatChunk {
    choices: Option<Vec<ChoiceChunk>>,
}

#[derive(Deserialize)]
struct ChoiceChunk {
    delta: Option<DeltaContent>,
}

#[derive(Deserialize)]
struct DeltaContent {
    content: Option<String>,
}

pub async fn generate(
    endpoint: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let url = format!("{endpoint}/v1/chat/completions");
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
        max_tokens: 512,
        temperature: 0.1,
        stop: vec!["\n\n\n".to_string()],
        chat_template_kwargs: ChatTemplateKwargs {
            enable_thinking: false,
        },
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to LLM server at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM server returned {status}: {body}"));
    }

    let mut full_response = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let payload = line
                .strip_prefix("data:")
                .map(str::trim)
                .unwrap_or(line);

            if payload == "[DONE]" {
                eprintln!();
                return Ok(full_response);
            }

            match serde_json::from_str::<ChatChunk>(payload) {
                Ok(chunk) => {
                    if let Some(choices) = &chunk.choices {
                        if let Some(choice) = choices.first() {
                            if let Some(content) = choice
                                .delta
                                .as_ref()
                                .and_then(|d| d.content.as_ref())
                            {
                                eprint!("{content}");
                                full_response.push_str(content);
                            }
                        }
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
            model: "qwen3-4b".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            stream: true,
            max_tokens: 512,
            temperature: 0.1,
            stop: vec![],
            chat_template_kwargs: ChatTemplateKwargs {
                enable_thinking: false,
            },
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(
            json.pointer("/chat_template_kwargs/enable_thinking"),
            Some(&serde_json::Value::Bool(false))
        );
    }
}

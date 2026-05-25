use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: GenerateOptions,
    keep_alive: String,
}

#[derive(Serialize)]
struct GenerateOptions {
    num_predict: u32,
    temperature: f32,
    stop: Vec<String>,
}

#[derive(Deserialize)]
struct GenerateChunk {
    response: String,
    done: bool,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, Value>,
}

pub async fn generate(
    endpoint: &str,
    model: &str,
    prompt: &str,
    keep_alive: &str,
) -> Result<String, String> {
    let url = format!("{endpoint}/api/generate");
    let client = Client::new();

    let request = GenerateRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        stream: true,
        options: GenerateOptions {
            num_predict: 256,
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
            match serde_json::from_str::<GenerateChunk>(line) {
                Ok(chunk) => {
                    eprint!("{}", chunk.response);
                    full_response.push_str(&chunk.response);
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

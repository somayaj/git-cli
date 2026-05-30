use crate::config::Backend;
use crate::{ollama, openai};

pub enum DetectedBackend {
    Ollama,
    OpenAi,
}

pub struct LlmStatus {
    pub backend: DetectedBackend,
    pub detail: String,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())
}

async fn probe_ollama(endpoint: &str, client: &reqwest::Client) -> bool {
    let url = format!("{endpoint}/api/tags");
    matches!(
        client.get(&url).send().await,
        Ok(resp) if resp.status().is_success()
    )
}

async fn probe_openai(endpoint: &str, client: &reqwest::Client) -> bool {
    let url = format!("{endpoint}/v1/models");
    matches!(
        client.get(&url).send().await,
        Ok(resp) if resp.status().is_success()
    )
}

pub async fn detect(endpoint: &str, preferred: Backend) -> Result<LlmStatus, String> {
    let client = http_client()?;

    match preferred {
        Backend::Ollama => {
            if probe_ollama(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::Ollama,
                    detail: format!("Ollama reachable at {endpoint}"),
                })
            } else {
                Err(format!(
                    "Ollama not reachable at {endpoint} (GET /api/tags failed)"
                ))
            }
        }
        Backend::Openai => {
            if probe_openai(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::OpenAi,
                    detail: format!("OpenAI-compatible server at {endpoint}"),
                })
            } else {
                Err(format!(
                    "OpenAI-compatible server not reachable at {endpoint} (GET /v1/models failed)"
                ))
            }
        }
        Backend::Auto => {
            if probe_ollama(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::Ollama,
                    detail: format!("Ollama reachable at {endpoint}"),
                })
            } else if probe_openai(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::OpenAi,
                    detail: format!(
                        "OpenAI-compatible server (llama.cpp) at {endpoint}"
                    ),
                })
            } else {
                Err(format!(
                    "No LLM server reachable at {endpoint} (tried Ollama /api/tags and OpenAI /v1/models)"
                ))
            }
        }
    }
}

pub async fn generate(
    endpoint: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    keep_alive: &str,
    backend: Backend,
) -> Result<String, String> {
    let detected = detect(endpoint, backend).await?;
    match detected.backend {
        DetectedBackend::Ollama => {
            ollama::generate(endpoint, model, system_prompt, user_prompt, keep_alive).await
        }
        DetectedBackend::OpenAi => {
            openai::generate(endpoint, model, system_prompt, user_prompt).await
        }
    }
}

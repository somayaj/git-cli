use crate::config::{self, Backend};
use crate::{ollama, openai};

pub enum DetectedBackend {
    MistralrsHttp,
    Ollama,
    OpenAi,
}

pub struct LlmStatus {
    pub backend: DetectedBackend,
    pub detail: String,
    pub http_endpoint: String,
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

pub async fn detect(
    endpoint: &str,
    endpoint_ollama: &str,
    preferred: Backend,
) -> Result<LlmStatus, String> {
    let client = http_client()?;

    match preferred {
        Backend::Ollama => {
            if probe_ollama(endpoint_ollama, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::Ollama,
                    detail: format!("Ollama reachable at {endpoint_ollama}"),
                    http_endpoint: endpoint_ollama.to_string(),
                })
            } else {
                Err(format!(
                    "Ollama not reachable at {endpoint_ollama} (GET /api/tags failed)"
                ))
            }
        }
        Backend::Openai => {
            if probe_openai(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::OpenAi,
                    detail: format!("OpenAI-compatible server at {endpoint}"),
                    http_endpoint: endpoint.to_string(),
                })
            } else {
                Err(format!(
                    "OpenAI-compatible server not reachable at {endpoint} (GET /v1/models failed)"
                ))
            }
        }
        Backend::MistralrsHttp => {
            if probe_openai(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::MistralrsHttp,
                    detail: format!("mistral.rs server at {endpoint}"),
                    http_endpoint: endpoint.to_string(),
                })
            } else {
                Err(format!(
                    "mistral.rs server not reachable at {endpoint} (GET /v1/models failed). \
                     Start: mistralrs serve -m {}",
                    config::HF_QWEN25_3B
                ))
            }
        }
        Backend::Auto => {
            // Prefer Ollama when /api/tags responds (Ollama also exposes /v1/models).
            if probe_ollama(endpoint_ollama, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::Ollama,
                    detail: format!("Ollama reachable at {endpoint_ollama}"),
                    http_endpoint: endpoint_ollama.to_string(),
                })
            } else if probe_openai(endpoint, &client).await {
                Ok(LlmStatus {
                    backend: DetectedBackend::MistralrsHttp,
                    detail: format!("mistral.rs / OpenAI-compatible server at {endpoint}"),
                    http_endpoint: endpoint.to_string(),
                })
            } else {
                Err(format!(
                    "No LLM server reachable (tried Ollama at {endpoint_ollama}, \
                     OpenAI-compatible at {endpoint}). \
                     Start: ollama serve && ollama pull qwen2.5:3b \
                     or mistralrs serve -m {}",
                    config::HF_QWEN25_3B
                ))
            }
        }
    }
}

fn server_model_name(model: &str, detected: &DetectedBackend) -> String {
    match detected {
        DetectedBackend::Ollama => model.to_string(),
        DetectedBackend::MistralrsHttp | DetectedBackend::OpenAi => {
            config::resolve_model_for_server(model)
        }
    }
}

pub async fn generate(
    endpoint: &str,
    endpoint_ollama: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    keep_alive: &str,
    backend: Backend,
) -> Result<String, String> {
    let detected = detect(endpoint, endpoint_ollama, backend).await?;
    let model = server_model_name(model, &detected.backend);
    let http = &detected.http_endpoint;

    match detected.backend {
        DetectedBackend::Ollama => {
            ollama::generate(http, &model, system_prompt, user_prompt, keep_alive).await
        }
        DetectedBackend::MistralrsHttp | DetectedBackend::OpenAi => {
            openai::generate(http, &model, system_prompt, user_prompt).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_qwen_tag_to_hf_id() {
        assert_eq!(
            config::resolve_model_for_server("qwen2.5:3b"),
            config::HF_QWEN25_3B
        );
        assert_eq!(
            config::resolve_model_for_server("custom-model"),
            "custom-model"
        );
    }
}

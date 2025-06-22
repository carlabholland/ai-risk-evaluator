// --- AI integration code start ---
use reqwest::Client;
use anyhow::Result;

async fn analyze_risks_ai(api_key: &str, project_text: &str) -> Result<Vec<RiskItem>> {
    let client = Client::new();

    let system_msg = serde_json::json!({
        "role": "system",
        "content": "You are a risk evaluator assistant. Extract project risks with their severity (low, medium, high) and suggested mitigation strategies in JSON format as an array of objects with fields: severity, category, mitigation."
    });

    let user_msg = serde_json::json!({
        "role": "user",
        "content": format!("Analyze the following project description and return risks:\n\n{}", project_text)
    });

    let request_body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [system_msg, user_msg],
        "max_tokens": 500,
        "temperature": 0.3,
    });

    #[cfg(debug_assertions)]
    println!("📤 Request body: {:?}", request_body);

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await?;

    let resp_json = resp.json::<serde_json::Value>().await?;

    println!("📥 OpenAI raw response: {:?}", resp_json);

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No content in AI response"))?;

    println!("📄 Extracted content: {}", content);

    let start = content.find('[').ok_or_else(|| anyhow::anyhow!("No JSON array found"))?;
    let end = content.rfind(']').ok_or_else(|| anyhow::anyhow!("No JSON array end found"))?;

    let json_str = &content[start..=end];

    println!("🔍 JSON string to parse: {}", json_str);

    let risks: Vec<RiskItem> = serde_json::from_str(json_str)?;

    Ok(risks)
}
// --- AI integration code end ---

use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use dotenv::dotenv;
use std::env;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Deserialize)]
struct RiskRequest {
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RiskItem {
    severity: String,
    category: String,
    mitigation: String,
}

#[derive(Debug, Serialize)]
struct RiskResponse {
    risks: Vec<RiskItem>,
}

#[derive(Clone)]
struct AppState {
    openai_api_key: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let openai_api_key = env::var("OPENAI_API_KEY")
    .unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            println!("⚠️ Using fallback API key for dev.");
            "fake-api-key".to_string()
        } else {
            panic!("❌ OPENAI_API_KEY not set in production!");
        }
    });

    let state = Arc::new(AppState { openai_api_key });

    println!("🔐 API Key loaded from environment.");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/evaluate", post(evaluate_risks))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("✅ Listening on http://{}", addr);

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn evaluate_risks(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RiskRequest>,
) -> Json<RiskResponse> {
    println!("📨 Received: {}", payload.description);

    match analyze_risks_ai(&state.openai_api_key, &payload.description).await {
        Ok(risks) => Json(RiskResponse { risks }),
        Err(e) => {
            eprintln!("❌ AI call error: {:?}", e);
            let fallback = vec![
                RiskItem {
                    severity: "High".to_string(),
                    category: "Timeline".to_string(),
                    mitigation: "Add buffer time and revalidate milestones.".to_string(),
                },
                RiskItem {
                    severity: "Medium".to_string(),
                    category: "Dependencies".to_string(),
                    mitigation: "Check for alternatives and establish SLAs.".to_string(),
                },
            ];
            Json(RiskResponse { risks: fallback })
        }
    }
}

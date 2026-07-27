use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRef;
use tokio::sync::{broadcast, mpsc};

use virs_bot::auto::types::AutoCommand;
use virs_bot::grid::types::GridCommand;
use virs_error::{VirsError, VirsResult};
use virs_exchange::Exchanges;
use virs_market::{KlineEngine, OrderBookEngine};
use virs_strategy::llm_client::{call_llm_api, LlmCallResult};
use virs_strategy::prompt::PromptLoader;
use virs_types::position::EngineEvent;


#[async_trait]
pub trait EngineManager: Send + Sync {


    async fn ensure_started(&self, paper_mode: bool) -> VirsResult<()>;


    fn grid_cmd_tx(&self) -> Option<mpsc::Sender<GridCommand>>;


    fn auto_cmd_tx(&self) -> Option<mpsc::Sender<AutoCommand>>;


    fn paper_mode(&self) -> Option<bool>;


    fn restore_error(&self) -> Option<String>;


    async fn register_paper_symbol(&self, exchange: String, symbol: String);


    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>>;


    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_types::position::Position>;


    async fn restore_if_needed(&self) -> VirsResult<()>;


    async fn shutdown(&self);
}


#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub engine_manager: Arc<dyn EngineManager>,
    pub http_client: reqwest::Client,
    pub exchange_registry: Arc<Exchanges>,
    pub kline_engine: Arc<KlineEngine>,
    pub orderbook_engine: Arc<OrderBookEngine>,
    pub encryption_key: String,
    pub llm_key: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,


    pub http_timeout_secs: u64,

    pub http_connect_timeout_secs: u64,

    pub http_pool_max_idle_per_host: usize,

    pub listenkey_keepalive_futures_secs: u64,

    /// 全局共享的策略模板加载器（启动时一次性加载，所有 handler 复用）。
    pub prompt_loader: PromptLoader,
}

impl AppState {
    /// 从 DB 读取最新一条 LLM 凭证 → 解密 → 解析 provider → 返回 `(api_key, base_url, model)`。
    ///
    /// 供 API handler 层的 LLM 调用使用（optimize / explain / recommend_strategy / generate 等）。
    pub async fn resolve_llm_credentials(&self) -> VirsResult<(String, String, String)> {
        let row: Option<(String, String)> = sqlx::query_as(
            r#"SELECT provider, encrypted_api_key
               FROM qd_ai_credentials ORDER BY created_at DESC LIMIT 1"#,
        )
        .fetch_optional(&self.db_pool)
        .await?;

        match row {
            Some((provider, encrypted_key)) => {
                let api_key =
                    virs_utils::crypto::decrypt_with_key(&encrypted_key, &self.llm_key)?;
                let base_url = virs_types::llm::resolve_provider_base_url(&provider)
                    .ok_or_else(|| VirsError::config(format!("Unknown AI provider: {provider}")))?
                    .to_string();
                let model = virs_types::llm::resolve_provider_model(&provider)
                    .ok_or_else(|| VirsError::config(format!("Unknown AI provider: {provider}")))?
                    .to_string();
                Ok((api_key, base_url, model))
            }
            None => Err(VirsError::unauthorized(
                "No AI API key configured. Set AI credentials first.",
            )),
        }
    }

    /// 解析凭证 → 调用 LLM API 的统一入口。
    ///
    /// 供所有 API handler 复用,避免各 handler 重复 `resolve_llm_credentials + call_llm_api`。
    /// `provider_name` 仅用于错误日志标识调用方(如 "auto-ai" / "strategy-selection")。
    pub async fn call_llm(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        provider_name: &str,
    ) -> VirsResult<LlmCallResult> {
        let (api_key, base_url, model) = self.resolve_llm_credentials().await?;
        let result = call_llm_api(
            &self.http_client,
            &api_key,
            &base_url,
            &model,
            system_prompt,
            user_prompt,
            provider_name,
        )
        .await?;
        Ok(result)
    }
}

impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}

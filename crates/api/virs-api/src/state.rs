use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRef;
use tokio::sync::{broadcast, mpsc};

use virs_type::ChatCommand;
use virs_error::{VirsError, VirsResult};
use virs_exchange::Exchanges;
use virs_type::{KlineEngineHandle, OrderBookEngineHandle};
use virs_llm::{call_llm_api, LlmCallResult};
use virs_prompt::PromptLoader;
use virs_type::EngineEvent;


/* 引擎管理trait：抽象持仓引擎和交易bot引擎的生命周期管理，供API层通过trait object调用 */
#[async_trait]
pub trait EngineManager: Send + Sync {


    async fn ensure_started(&self, paper_mode: bool) -> VirsResult<()>;


    fn chat_cmd_tx(&self) -> Option<mpsc::Sender<ChatCommand>>;


    fn paper_mode(&self) -> Option<bool>;


    fn restore_error(&self) -> Option<String>;


    fn pe_event_subscribe(&self) -> Option<broadcast::Receiver<EngineEvent>>;


    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<virs_type::Position>;


    async fn restore_if_needed(&self) -> VirsResult<()>;


    async fn shutdown(&self);
}


#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub engine_manager: Arc<dyn EngineManager>,
    pub http_client: reqwest::Client,
    pub exchange_registry: Arc<Exchanges>,
    pub kline_engine: Arc<dyn KlineEngineHandle>,
    pub orderbook_engine: Arc<dyn OrderBookEngineHandle>,
    pub encryption_key: String,
    pub llm_key: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,


    pub http_timeout_secs: u64,

    pub http_connect_timeout_secs: u64,

    pub http_pool_max_idle_per_host: usize,

    pub listenkey_keepalive_futures_secs: u64,


    pub prompt_loader: PromptLoader,
}

impl AppState {


    /* 解析LLM凭据：从数据库查询最新AI凭据，解密API Key，并匹配provider对应的base_url和model */
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
                    virs_utils::decrypt_with_key(&encrypted_key, &self.llm_key)?;
                let base_url = virs_type::LlmProviderConfig::for_provider(&provider)
                    .map(|c| c.base_url)
                    .ok_or_else(|| VirsError::config(format!("Unknown AI provider: {provider}")))?
                    .to_string();
                let model = virs_type::LlmProviderConfig::for_provider(&provider)
                    .map(|c| c.default_model)
                    .ok_or_else(|| VirsError::config(format!("Unknown AI provider: {provider}")))?
                    .to_string();
                Ok((api_key, base_url, model))
            }
            None => Err(VirsError::unauthorized(
                "No AI API key configured. Set AI credentials first.",
            )),
        }
    }


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

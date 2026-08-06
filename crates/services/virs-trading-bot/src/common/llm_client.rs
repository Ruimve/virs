use std::sync::Arc;

use uuid::Uuid;
use virs_error::BotResult;

use virs_type::{CredentialStore, LlmProviderResolver};
use virs_tactical_bot::{call_llm_api, create_llm_http_client, LlmCallResult};

/// 共享的 LLM 客户端：封装凭证加载 + provider 解析 + HTTP 调用。
///
/// `AutoAiService` 内部持有一个实例，
/// 避免重复实现 `load_credentials → resolve → call_llm_api` 链路。
pub struct LlmClient {
    http_client: reqwest::Client,
    llm_resolver: Arc<dyn LlmProviderResolver>,
    credential_store: Arc<dyn CredentialStore>,
}

impl LlmClient {
    pub fn new(
        llm_resolver: Arc<dyn LlmProviderResolver>,
        credential_store: Arc<dyn CredentialStore>,
        llm_timeout: std::time::Duration,
    ) -> Self {
        Self {
            http_client: create_llm_http_client(llm_timeout),
            llm_resolver,
            credential_store,
        }
    }

    /// 检查指定用户是否有可用的 LLM 凭证。
    pub async fn is_available_for_user(&self, user_id: Uuid) -> bool {
        if self.llm_resolver.is_available() {
            return true;
        }
        match self.credential_store.load_credentials(user_id).await {
            Ok(creds) => !creds.is_empty(),
            Err(_) => false,
        }
    }

    /// 加载用户凭证 → 解析 provider → 调用 LLM API。
    ///
    /// `provider_name` 仅用于错误日志标识调用方（如 "auto-ai"）。
    pub async fn call(
        &self,
        user_id: Uuid,
        system_prompt: &str,
        user_prompt: &str,
        provider_name: &str,
    ) -> BotResult<LlmCallResult> {
        let user_creds = self.credential_store.load_credentials(user_id).await?;
        let (api_key, base_url, model, _provider) = self.llm_resolver.resolve(&user_creds)?;

        call_llm_api(
            &self.http_client,
            &api_key,
            &base_url,
            &model,
            system_prompt,
            user_prompt,
            provider_name,
        )
        .await
    }
}

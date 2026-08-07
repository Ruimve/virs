use std::sync::Arc;

use uuid::Uuid;
use virs_error::BotResult;

use virs_type::{CredentialStore, LlmProviderResolver};
use virs_llm::{call_llm_api, create_llm_http_client, LlmCallResult};


pub(crate) struct LlmClient {
    http_client: reqwest::Client,
    llm_resolver: Arc<dyn LlmProviderResolver>,
    credential_store: Arc<dyn CredentialStore>,
}

impl LlmClient {
    pub(crate) fn new(
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


    pub(crate) async fn is_available_for_user(&self, user_id: Uuid) -> bool {
        if self.llm_resolver.is_available() {
            return true;
        }
        match self.credential_store.load_credentials(user_id).await {
            Ok(creds) => !creds.is_empty(),
            Err(_) => false,
        }
    }


    pub(crate) async fn call(
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

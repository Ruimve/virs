use async_trait::async_trait;
use uuid::Uuid;

/** 账户余额信息 */
#[derive(Debug, Clone, Default)]
pub struct AccountBalance {
    pub total: f64,
    pub free: f64,
    pub used: f64,
}

/** 用户凭证存储端口 */
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load_credentials(&self, user_id: Uuid) -> anyhow::Result<Vec<(String, String)>>;
}

/** LLM 提供商解析器端口 */
pub trait LlmProviderResolver: Send + Sync {
    fn is_available(&self) -> bool;
    fn resolve(
        &self,
        user_credentials: &[(String, String)],
    ) -> anyhow::Result<(String, String, String, String)>;
}

use async_trait::async_trait;
use uuid::Uuid;
use virs_database::PgPool;
use virs_database as db;
use virs_error::{BotError, BotResult};
use virs_type::CredentialStore;

/* 凭据存储适配器：从PostgreSQL加载加密的AI凭据并解密，供BotEngine使用 */
pub struct PgCredentialStore {
    db: PgPool,
    encryption_key: [u8; 32],
}

impl PgCredentialStore {
    pub fn new(db: PgPool, encryption_key: [u8; 32]) -> Self {
        Self { db, encryption_key }
    }
}

#[async_trait]
impl CredentialStore for PgCredentialStore {
    async fn load_credentials(
        &self,
        user_id: Uuid,
    ) -> BotResult<Vec<(String, String, Option<String>)>> {
        let rows = db::load_ai_credentials_for_bot(&self.db, user_id)
            .await
            .map_err(|e| BotError::Credential(format!("DB query failed: {}", e)))?;

        let mut result = Vec::new();
        for (provider, encrypted_api_key, model) in rows {
            let decrypted =
                virs_utils::decrypt(&encrypted_api_key, &self.encryption_key)
                    .map_err(|e| BotError::Credential(format!("Decryption failed: {}", e)))?;
            result.push((provider, decrypted, model));
        }
        Ok(result)
    }
}

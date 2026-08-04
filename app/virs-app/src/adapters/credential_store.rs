use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use virs_error::{BotError, BotResult};
use virs_type::bot::CredentialStore;

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
        #[derive(Debug, sqlx::FromRow)]
        struct Row {
            provider: String,
            encrypted_api_key: String,
            model: Option<String>,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT provider, encrypted_api_key, model FROM qd_ai_credentials WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await
        .map_err(|e| BotError::Credential(format!("DB query failed: {}", e)))?;

        let mut result = Vec::new();
        for row in rows {
            let decrypted =
                virs_utils::crypto::decrypt(&row.encrypted_api_key, &self.encryption_key)
                    .map_err(|e| BotError::Credential(format!("Decryption failed: {}", e)))?;
            result.push((row.provider, decrypted, row.model));
        }
        Ok(result)
    }
}

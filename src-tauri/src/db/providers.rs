use super::Database;
use crate::models::AiProviderConfig;
use rusqlite::{params, OptionalExtension};

impl Database {
    pub fn list_providers(&self) -> Result<Vec<AiProviderConfig>, String> {
        self.list_json("SELECT payload_json FROM ai_providers ORDER BY rowid")
    }

    pub fn provider_by_id(&self, id: &str) -> Result<Option<AiProviderConfig>, String> {
        let connection = self.connect()?;
        let payload = connection
            .query_row(
                "SELECT payload_json FROM ai_providers WHERE id=?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
            .transpose()
    }

    pub fn default_provider(&self) -> Result<Option<AiProviderConfig>, String> {
        Ok(self
            .list_providers()?
            .into_iter()
            .find(|provider| provider.is_default && provider.verified))
    }

    pub fn save_provider(&self, provider: &AiProviderConfig) -> Result<(), String> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        if provider.is_default {
            let mut providers = {
                let mut statement = transaction
                    .prepare("SELECT payload_json FROM ai_providers")
                    .map_err(|error| error.to_string())?;
                let rows = statement
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(|error| error.to_string())?;
                rows.filter_map(Result::ok)
                    .filter_map(|payload| serde_json::from_str::<AiProviderConfig>(&payload).ok())
                    .collect::<Vec<_>>()
            };
            for item in &mut providers {
                if item.id != provider.id && item.is_default {
                    item.is_default = false;
                    let payload = serde_json::to_string(item).map_err(|error| error.to_string())?;
                    transaction
                        .execute(
                            "UPDATE ai_providers SET payload_json=?1 WHERE id=?2",
                            params![payload, item.id],
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        let payload = serde_json::to_string(provider).map_err(|error| error.to_string())?;
        transaction.execute("INSERT INTO ai_providers(id, payload_json) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json", params![provider.id, payload]).map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }
}

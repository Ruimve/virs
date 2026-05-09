use crate::exchange::Exchange;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub struct ExchangeRegistry {
    exchanges: Arc<DashMap<String, Box<dyn Exchange>>>,
}

impl ExchangeRegistry {
    pub fn new() -> Self {
        Self {
            exchanges: Arc::new(DashMap::new()),
        }
    }

    pub fn register(&self, exchange: Box<dyn Exchange>) {
        let name = exchange.name().to_string();
        let mt = exchange.market_type();
        let key = format!("{}:{}", name, mt);
        info!("Registered exchange: {} (key={})", name, key);
        self.exchanges.insert(key, exchange);
    }

    pub fn register_for_user(&self, exchange: Box<dyn Exchange>, user_id: Uuid) -> String {
        let raw_name = exchange.name().to_string();
        let mt = exchange.market_type();
        let scoped_name = format!("{}:{}:{}", raw_name, mt, user_id);
        info!("Registered exchange '{}' ({:?}) for user {}", raw_name, mt, user_id);
        self.exchanges.insert(scoped_name.clone(), exchange);
        scoped_name
    }

    pub fn get(&self, name: &str) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn Exchange>>> {
        self.exchanges.get(name)
    }

    pub fn registered_names(&self) -> Vec<String> {
        self.exchanges.iter().map(|r| r.key().clone()).collect()
    }

    pub fn remove_user_exchange(&self, exchange_name: &str, market_type: &str, user_id: &str) {
        let key = format!("{}:{}:{}", exchange_name, market_type, user_id);
        self.exchanges.remove(&key);
    }
}

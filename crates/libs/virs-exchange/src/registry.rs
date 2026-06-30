//! Exchanges — manages named exchange instances.

use crate::Exchange;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::info;

pub struct Exchanges {
    exchanges: Arc<DashMap<String, Box<dyn Exchange>>>,
}

impl Default for Exchanges {
    fn default() -> Self {
        Self::new()
    }
}

impl Exchanges {
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

    pub fn get(
        &self,
        name: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, Box<dyn Exchange>>> {
        self.exchanges.get(name)
    }

    pub fn registered_names(&self) -> Vec<String> {
        self.exchanges.iter().map(|r| r.key().clone()).collect()
    }
}

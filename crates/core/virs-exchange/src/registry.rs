use virs_type::ExchangePe;
use dashmap::DashMap;
use std::sync::Arc;

pub struct Exchanges {
    exchanges: Arc<DashMap<String, Arc<dyn ExchangePe>>>,
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


    pub fn register(&self, exchange: Box<dyn ExchangePe>) {
        let name = exchange.name().to_string();
        let mt = exchange.market_type();
        let key = format!("{}:{}", name, mt);
        let exchange: Arc<dyn ExchangePe> = Arc::from(exchange);
        self.exchanges.insert(key, exchange);
    }

    pub fn get(
        &self,
        name: &str,
    ) -> Option<dashmap::mapref::one::Ref<'_, String, Arc<dyn ExchangePe>>> {
        self.exchanges.get(name)
    }

    pub fn registered_names(&self) -> Vec<String> {
        self.exchanges.iter().map(|r| r.key().clone()).collect()
    }


    pub fn get_perpetual(&self) -> Option<Arc<dyn ExchangePe>> {
        self.exchanges
            .iter()
            .find(|r| r.key().contains("perpetual"))
            .map(|r| r.value().clone())
    }
}

use virs_error::VirsResult;

use crate::position::Position;
use crate::{CcxtOrder};

/* 持仓持久化trait：定义从订单数据重建持仓、持久化订单等接口。
 * trait定义在virs-type(L1)以便virs-database(L1)实现，virs-position(L3)通过依赖注入使用 */
#[async_trait::async_trait]
pub trait PositionPersistence: Send + Sync {
    async fn get_positions_from_orders(&self, exchange: &str) -> VirsResult<Vec<Position>>;

    async fn persist_order(&self, order: &CcxtOrder) -> VirsResult<()>;

    async fn persist_rejected_order(&self, order: &CcxtOrder, reason: &str) -> VirsResult<()>;

    async fn get_active_orders(&self) -> VirsResult<Vec<CcxtOrder>>;
}

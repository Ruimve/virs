use super::types::{Position, Trade, TradeType};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// ============================================================================
// PnlSnapshot
// ============================================================================

/// 盈亏快照
#[derive(Debug, Clone)]
pub struct PnlSnapshot {
    /// 快照时间
    pub timestamp: DateTime<Utc>,
    /// 当前权益（初始权益 + 已实现 + 未实现）
    pub equity: f64,
    /// 未实现盈亏
    pub unrealized_pnl: f64,
    /// 已实现盈亏
    pub realized_pnl: f64,
    /// 总盈亏（已实现 + 未实现）
    pub total_pnl: f64,
    /// 最大回撤（从峰值到当前的回撤比例）
    pub max_drawdown: f64,
    /// 当前持仓数量
    pub open_positions_count: usize,
    /// 胜率（盈利交易 / 总交易），无交易时为 None
    pub win_rate: Option<f64>,
    /// 平均盈亏比（平均盈利 / 平均亏损的绝对值），无亏损交易时为 None
    pub avg_pnl_ratio: Option<f64>,
    /// 总盈亏比（总盈利 / 总亏损的绝对值），无亏损交易时为 None
    pub pnl_ratio: Option<f64>,
}

// ============================================================================
// PnlTracker
// ============================================================================

/// 盈亏追踪器
///
/// 负责追踪未实现盈亏、已实现盈亏、胜率、回撤等指标。
pub struct PnlTracker {
    peak_equity: f64,
    total_realized_pnl: f64,
    total_trades: u32,
    profit_trades: u32,
    total_cost: f64,
    total_profit_amount: f64,
    total_loss_amount: f64,
    initial_equity: f64,
    consecutive_losses: u32,
}

impl PnlTracker {
    /// 创建新的盈亏追踪器。
    ///
    /// `initial_equity` 为账户初始权益。
    pub fn new(initial_equity: f64) -> Self {
        Self {
            peak_equity: initial_equity,
            total_realized_pnl: 0.0,
            total_trades: 0,
            profit_trades: 0,
            total_cost: 0.0,
            total_profit_amount: 0.0,
            total_loss_amount: 0.0,
            initial_equity,
            consecutive_losses: 0,
        }
    }

    // -----------------------------------------------------------------------
    // 更新未实现盈亏
    // -----------------------------------------------------------------------

    /// 更新未实现盈亏（同步循环中调用）。
    ///
    /// 遍历所有持仓，根据当前价格计算未实现盈亏，更新峰值权益，返回快照。
    pub fn update_unrealized(
        &mut self,
        positions: &[&Position],
        current_prices: &HashMap<String, f64>,
    ) -> PnlSnapshot {
        let mut unrealized_pnl = 0.0;

        for pos in positions {
            // 优先使用传入的当前价格，否则使用持仓自身的 current_price
            let price = current_prices
                .get(&pos.symbol)
                .copied()
                .unwrap_or(pos.current_price);

            let pos_pnl = match pos.side {
                super::types::PositionSide::Long => (price - pos.entry_price) * pos.size,
                super::types::PositionSide::Short => (pos.entry_price - price) * pos.size,
                super::types::PositionSide::Both => {
                    // Both 模式下用持仓自身的 unrealized_pnl
                    pos.unrealized_pnl
                }
            };

            unrealized_pnl += pos_pnl;
        }

        let equity = self.initial_equity + self.total_realized_pnl + unrealized_pnl;

        // 更新峰值权益
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        self.snapshot(unrealized_pnl, positions.len())
    }

    // -----------------------------------------------------------------------
    // 记录成交
    // -----------------------------------------------------------------------

    /// 记录一笔成交（成交回调中调用）。
    ///
    /// 更新已实现盈亏、交易次数、盈利次数、开仓成本等。
    pub fn record_trade(&mut self, trade: &Trade) {
        self.total_realized_pnl += trade.pnl;
        self.total_trades += 1;

        if trade.pnl >= 0.0 {
            self.profit_trades += 1;
            self.total_profit_amount += trade.pnl;
            self.consecutive_losses = 0;
        } else {
            self.total_loss_amount += trade.pnl.abs();
            self.consecutive_losses += 1;
        }

        if trade.trade_type == TradeType::Open {
            self.total_cost += trade.price * trade.amount + trade.fee;
        }
    }

    // -----------------------------------------------------------------------
    // 获取快照
    // -----------------------------------------------------------------------

    /// 获取当前快照。
    pub fn snapshot(&self, unrealized_pnl: f64, open_positions_count: usize) -> PnlSnapshot {
        let equity = self.initial_equity + self.total_realized_pnl + unrealized_pnl;
        let total_pnl = self.total_realized_pnl + unrealized_pnl;

        // 最大回撤
        let max_drawdown = if self.peak_equity > 0.0 {
            (self.peak_equity - equity) / self.peak_equity
        } else {
            0.0
        };

        // 胜率
        let win_rate = if self.total_trades > 0 {
            Some(self.profit_trades as f64 / self.total_trades as f64)
        } else {
            None
        };

        // 盈亏比
        let pnl_ratio = if self.total_loss_amount > 0.0 {
            Some(self.total_profit_amount / self.total_loss_amount)
        } else {
            None
        };

        // 平均盈亏比
        let avg_pnl_ratio = if self.total_loss_amount > 0.0 && self.profit_trades > 0 {
            let loss_trades = self.total_trades - self.profit_trades;
            if loss_trades > 0 {
                let avg_profit = self.total_profit_amount / self.profit_trades as f64;
                let avg_loss = self.total_loss_amount / loss_trades as f64;
                Some(avg_profit / avg_loss)
            } else {
                None
            }
        } else {
            None
        };

        PnlSnapshot {
            timestamp: Utc::now(),
            equity,
            unrealized_pnl,
            realized_pnl: self.total_realized_pnl,
            total_pnl,
            max_drawdown,
            open_positions_count,
            win_rate,
            avg_pnl_ratio,
            pnl_ratio,
        }
    }

    // -----------------------------------------------------------------------
    // 从快照恢复
    // -----------------------------------------------------------------------

    /// 从数据库恢复状态。
    ///
    /// 用于引擎重启后从持久化数据中恢复盈亏追踪器的状态。
    pub fn restore_from_snapshot(
        &mut self,
        peak_equity: f64,
        realized_pnl: f64,
        total_trades: u32,
        profit_trades: u32,
        total_cost: f64,
        consecutive_losses: u32,
    ) {
        self.peak_equity = peak_equity;
        self.total_realized_pnl = realized_pnl;
        self.total_trades = total_trades;
        self.profit_trades = profit_trades;
        self.total_cost = total_cost;
        self.consecutive_losses = consecutive_losses;
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// 获取历史最高权益。
    pub fn peak_equity(&self) -> f64 {
        self.peak_equity
    }

    pub fn equity(&self) -> f64 {
        self.initial_equity + self.total_realized_pnl
    }

    /// 获取累计已实现盈亏。
    pub fn total_realized_pnl(&self) -> f64 {
        self.total_realized_pnl
    }

    /// 获取总交易次数。
    pub fn total_trades(&self) -> u32 {
        self.total_trades
    }

    /// 获取盈利交易次数。
    pub fn profit_trades(&self) -> u32 {
        self.profit_trades
    }

    /// 获取累计开仓成本。
    pub fn total_cost(&self) -> f64 {
        self.total_cost
    }

    pub fn consecutive_losses(&self) -> u32 {
        self.consecutive_losses
    }
}



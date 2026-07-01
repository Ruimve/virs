//! PnL tracker for position engine.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use virs_types::enums::PositionSide;
use virs_types::position::Position;

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
    /// 最大回撤（从峰值到当前的回撤比例）
    pub max_drawdown: f64,
}

// ============================================================================
// Idempotent functions
// ============================================================================

/// 计算未实现盈亏（纯函数）。
pub fn calc_unrealized_pnl(
    positions: &[&Position],
    current_prices: &HashMap<String, f64>,
) -> f64 {
    let mut unrealized_pnl = 0.0;

    for pos in positions {
        let price = current_prices
            .get(&pos.symbol)
            .copied()
            .unwrap_or(pos.current_price);

        let pos_pnl = match pos.side {
            PositionSide::Long => (price - pos.entry_price) * pos.size,
            PositionSide::Short => (pos.entry_price - price) * pos.size,
            PositionSide::Both => pos.unrealized_pnl,
        };

        unrealized_pnl += pos_pnl;
    }

    unrealized_pnl
}

/// 计算回撤百分比（纯函数）。
pub fn calc_drawdown_pct(peak_equity: f64, current_equity: f64) -> f64 {
    if peak_equity > 0.0 {
        (peak_equity - current_equity) / peak_equity
    } else {
        0.0
    }
}

// ============================================================================
// PnlTracker
// ============================================================================

/// 盈亏追踪器
pub struct PnlTracker {
    peak_equity: f64,
    total_realized_pnl: f64,
    initial_equity: f64,
    last_unrealized_pnl: f64,
}

impl PnlTracker {
    /// 创建新的盈亏追踪器。
    pub fn new(initial_equity: f64) -> Self {
        Self {
            peak_equity: initial_equity,
            total_realized_pnl: 0.0,
            initial_equity,
            last_unrealized_pnl: 0.0,
        }
    }

    /// 更新未实现盈亏（同步循环中调用）。
    pub fn update_unrealized(
        &mut self,
        positions: &[&Position],
        current_prices: &HashMap<String, f64>,
    ) -> PnlSnapshot {
        let unrealized_pnl = calc_unrealized_pnl(positions, current_prices);

        let equity = self.initial_equity + self.total_realized_pnl + unrealized_pnl;

        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        self.last_unrealized_pnl = unrealized_pnl;
        self.snapshot(unrealized_pnl)
    }

    /// 记录一笔成交（成交回调中调用）。
    pub fn record_trade(&mut self, trade: &virs_types::position::Trade) {
        self.total_realized_pnl += trade.pnl;

        let equity = self.initial_equity + self.total_realized_pnl + self.last_unrealized_pnl;
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }
    }

    /// 获取当前快照。
    pub fn snapshot(&self, unrealized_pnl: f64) -> PnlSnapshot {
        let equity = self.initial_equity + self.total_realized_pnl + unrealized_pnl;
        let max_drawdown = calc_drawdown_pct(self.peak_equity, equity);

        PnlSnapshot {
            timestamp: Utc::now(),
            equity,
            max_drawdown,
        }
    }

    /// 获取历史最高权益。
    pub fn peak_equity(&self) -> f64 {
        self.peak_equity
    }

    pub fn equity(&self) -> f64 {
        self.initial_equity + self.total_realized_pnl
    }
}

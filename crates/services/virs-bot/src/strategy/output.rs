//! 统一策略输出类型。
//!
//! 设计目标：
//! - [`AutoDecision`] 与 [`GridAiDecision`] 各自实现 [`ToStrategyOutput`]，
//!   转换为统一的 [`StrategyOutput`]，供日志、风控拦截、审计、前端推送统一消费
//! - [`StrategyAction`] 用一个枚举涵盖 auto/grid 的全部动作语义，
//!   避免下游处理时再按策略类型分支
//! - 保留 `decision_raw`（LLM 原始 JSON）用于审计与回放
//!
//! 注意：本模块只定义"输出契约"，不改变 worker 的 `execute_decision` 内部逻辑。
//! 各策略的 `to_output` 是纯转换函数，可独立测试。
//!
//! [`AutoDecision`]: crate::auto::ai::AutoDecision
//! [`GridAiDecision`]: crate::grid::ai::GridAiDecision

use uuid::Uuid;

/// 统一策略动作。涵盖 auto 与 grid 两种策略的全部动作语义。
#[derive(Debug, Clone, PartialEq)]
pub enum StrategyAction {
    // ── Auto 策略动作 ──
    OpenLong,
    OpenShort,
    ClosePosition,
    Hold,

    // ── Grid 策略动作 ──
    /// 调整网格上下界（可能伴随 grid_count/profit_pct/qty 变化）
    AdjustGrid {
        upper_price: f64,
        lower_price: f64,
        grid_count: i32,
        grid_profit_pct: f64,
        quantity_per_grid: f64,
    },
    PauseGrid,
    RunGrid,
    ReducePosition,
}

impl StrategyAction {
    /// 动作的规范字符串名（用于日志、持久化、前端展示）。
    pub fn as_str(&self) -> &'static str {
        match self {
            StrategyAction::OpenLong => "open_long",
            StrategyAction::OpenShort => "open_short",
            StrategyAction::ClosePosition => "close_position",
            StrategyAction::Hold => "hold",
            StrategyAction::AdjustGrid { .. } => "adjust_grid",
            StrategyAction::PauseGrid => "pause_grid",
            StrategyAction::RunGrid => "run_grid",
            StrategyAction::ReducePosition => "reduce_position",
        }
    }

    /// 是否为"无操作"动作（不需要下单或状态变更）。
    pub fn is_noop(&self) -> bool {
        matches!(self, StrategyAction::Hold)
    }

    /// 是否为开仓类动作（需要风控前置拦截）。
    pub fn is_open_position(&self) -> bool {
        matches!(
            self,
            StrategyAction::OpenLong | StrategyAction::OpenShort
        )
    }

    /// 是否为网格结构变更类动作（需要持久化新参数）。
    pub fn is_grid_restructure(&self) -> bool {
        matches!(
            self,
            StrategyAction::AdjustGrid { .. }
                | StrategyAction::PauseGrid
                | StrategyAction::RunGrid
                | StrategyAction::ReducePosition
        )
    }
}

/// 统一策略输出。由具体策略的 Decision 转换而来。
#[derive(Debug, Clone)]
pub struct StrategyOutput {
    /// 规范动作
    pub action: StrategyAction,
    /// LLM 给出的理由
    pub reason: String,
    /// 置信度 [0.0, 1.0]
    pub confidence: f64,
    /// 市场状态分类（如 "trending_up" / "ranging" / "volatile"）
    pub market_regime: Option<String>,
    /// LLM 分析文本（用于日志/前端展示）
    pub analysis: Option<String>,
    /// 风险提示（用于日志/前端展示）
    pub risk_warning: Option<String>,
    /// 资金费率警告
    pub funding_rate_warning: Option<String>,
    /// 事件影响提示
    pub event_impact: Option<String>,
    /// LLM 原始返回 JSON（审计/回放用）
    pub decision_raw: serde_json::Value,
    /// 关联的 bot id（便于日志关联）
    pub bot_id: Option<Uuid>,
}

impl StrategyOutput {
    /// 构造一个 Hold 输出（用于 LLM 不可用、风控拦截降级等场景）。
    pub fn hold(bot_id: Option<Uuid>, reason: impl Into<String>) -> Self {
        Self {
            action: StrategyAction::Hold,
            reason: reason.into(),
            confidence: 0.0,
            market_regime: None,
            analysis: None,
            risk_warning: None,
            funding_rate_warning: None,
            event_impact: None,
            decision_raw: serde_json::json!({ "action": "hold", "reason": "fallback" }),
            bot_id,
        }
    }

    /// 是否为无操作。
    pub fn is_noop(&self) -> bool {
        self.action.is_noop()
    }

    /// 是否为开仓类动作。
    pub fn is_open_position(&self) -> bool {
        self.action.is_open_position()
    }
}

/// Decision → StrategyOutput 转换 trait。
///
/// 各策略的 Decision 类型实现此 trait，统一输出格式。
/// 实现为纯函数（不触碰 IO），可独立单元测试。
pub trait ToStrategyOutput {
    fn to_output(&self, raw: serde_json::Value, bot_id: Option<Uuid>) -> StrategyOutput;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o1_action_as_str_roundtrip() {
        assert_eq!(StrategyAction::OpenLong.as_str(), "open_long");
        assert_eq!(StrategyAction::Hold.as_str(), "hold");
        assert_eq!(
            StrategyAction::AdjustGrid {
                upper_price: 100.0,
                lower_price: 90.0,
                grid_count: 10,
                grid_profit_pct: 0.5,
                quantity_per_grid: 1.0,
            }
            .as_str(),
            "adjust_grid"
        );
    }

    #[test]
    fn o2_action_predicates() {
        assert!(StrategyAction::OpenLong.is_open_position());
        assert!(StrategyAction::OpenShort.is_open_position());
        assert!(!StrategyAction::ClosePosition.is_open_position());
        assert!(!StrategyAction::Hold.is_open_position());

        assert!(StrategyAction::Hold.is_noop());
        assert!(!StrategyAction::OpenLong.is_noop());

        assert!(StrategyAction::PauseGrid.is_grid_restructure());
        assert!(StrategyAction::RunGrid.is_grid_restructure());
        assert!(StrategyAction::ReducePosition.is_grid_restructure());
        assert!(StrategyAction::AdjustGrid {
            upper_price: 1.0,
            lower_price: 0.5,
            grid_count: 5,
            grid_profit_pct: 0.1,
            quantity_per_grid: 0.1
        }
        .is_grid_restructure());
        assert!(!StrategyAction::OpenLong.is_grid_restructure());
    }

    #[test]
    fn o3_hold_factory() {
        let out = StrategyOutput::hold(Some(Uuid::nil()), "LLM unavailable");
        assert!(out.is_noop());
        assert_eq!(out.reason, "LLM unavailable");
        assert_eq!(out.confidence, 0.0);
        assert!(out.bot_id.is_some());
    }
}

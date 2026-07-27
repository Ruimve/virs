import type { AutoBot, AutoBotStats, AnalysisLog } from '@/service/types';
import type { PositionWsEvent } from '@/service/ws';

/** 单边持仓指标（前端实时计算） */
export interface SideMetrics {
  hasPosition: boolean;
  unrealizedPnl: number;
  usedMargin: number;
  unrealizedPct: number;
  /** 粗略估算强平价（后端未提供 liquidation_price 时使用） */
  estimatedLiqPrice: number | null;
}

/** 账户汇总指标 */
export interface AccountMetrics {
  balance: number;
  usedMargin: number;
  freeMargin: number;
  marginRate: number;
}

/** 市场行 Props */
export interface MarketLineProps {
  symbol: string;
  exchange: string;
  latestPrice: number;
  changePct: number;
  /** bot 配置：杠杆倍数 */
  leverage: number;
  /** bot 配置：决策间隔（秒） */
  decideIntervalSecs: number;
  /** bot 运行状态 */
  status: string;
}

/** 账户条 Props */
export interface AccountBarProps {
  metrics: AccountMetrics;
}

/** AI 策略块 Props */
export interface StrategyBlockProps {
  bot: AutoBot;
  decision: AnalysisLog | null;
  stats: AutoBotStats | null;
  totalPnl: number;
  totalPnlPct: number;
}

/** 仓位卡 Props */
export interface PositionCardProps {
  side: 'long' | 'short';
  position: PositionWsEvent | null;
  metrics: SideMetrics;
  leverage: number;
  latestPrice: number;
  /** AI 最新决策（用于空仓时显示 AI 放弃理由） */
  decision: AnalysisLog | null;
  /** 该方向已实现盈亏（来自 bot.total_pnl 或 position.realizedPnl） */
  realizedPnl: number;
}

/** 上区 Props */
export interface UpperRegionProps {
  bot: AutoBot;
  latestPrice: number;
  marketSummary: { changePct: number; high: number; low: number; volume: number };
  decision: AnalysisLog | null;
  stats: AutoBotStats | null;
  accountMetrics: AccountMetrics;
  totalPnl: number;
  totalPnlPct: number;
}

/** 下区 Props */
export interface LowerRegionProps {
  bot: AutoBot;
  latestPrice: number;
  longPosition: PositionWsEvent | null;
  shortPosition: PositionWsEvent | null;
  longMetrics: SideMetrics;
  shortMetrics: SideMetrics;
  decision: AnalysisLog | null;
}

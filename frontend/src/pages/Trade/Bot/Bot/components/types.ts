import type { Bot, BotStats, AnalysisLog, StrategyDetail } from '@/service/types';
import type { PositionWsEvent } from '@/service/ws';

export interface SideMetrics {
  hasPosition: boolean;
  unrealizedPnl: number;
  usedMargin: number;
  unrealizedPct: number;

  estimatedLiqPrice: number | null;
}

export interface AccountMetrics {
  balance: number;
  usedMargin: number;
  freeMargin: number;
  marginRate: number;
}

export interface MarketLineProps {
  symbol: string;
  exchange: string;
  latestPrice: number;
  changePct: number;

  leverage: number;
}

export interface AccountBarProps {
  metrics: AccountMetrics;
}

export interface StrategyBlockProps {
  bot: Bot;

  strategy: StrategyDetail | null;
  decision: AnalysisLog | null;
  stats: BotStats | null;
  totalPnl: number;
  totalPnlPct: number;

  decideIntervalSecs: number;
}

export interface PositionCardProps {
  side: 'long' | 'short';
  position: PositionWsEvent | null;
  metrics: SideMetrics;
  leverage: number;
  latestPrice: number;

  decision: AnalysisLog | null;

  realizedPnl: number;
}

export interface UpperRegionProps {
  bot: Bot;

  strategy: StrategyDetail | null;
  latestPrice: number;
  marketSummary: { changePct: number; high: number; low: number; volume: number };
  decision: AnalysisLog | null;
  stats: BotStats | null;
  accountMetrics: AccountMetrics;
  totalPnl: number;
  totalPnlPct: number;
}

export interface LowerRegionProps {
  bot: Bot;
  latestPrice: number;
  longPosition: PositionWsEvent | null;
  shortPosition: PositionWsEvent | null;
  longMetrics: SideMetrics;
  shortMetrics: SideMetrics;
  decision: AnalysisLog | null;
}

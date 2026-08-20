import type {
  AnalysisLog,
  Bot,
  BotTrade,
  StrategyDetail,
  BotStats,
  KlineCandle,
} from '@/service/types';
import type { PositionWsEvent } from '@/service/ws';
import type { KlineChartHandle } from '@/components/Chart/KlineChart';
import type { SideMetrics, AccountMetrics } from '../components/types';

export interface MarketHeaderProps {
  bot: Bot;
  latestPrice: number;
  marketSummary: { changePct: number; high: number; low: number; volume: number };
  timeframe: string;
  onTimeframeChange: (tf: string) => void;
}

export interface AIPanelProps {
  decision: AnalysisLog | null;
  logs: AnalysisLog[];
  decideIntervalSecs: number;
}

export interface RiskPanelProps {
  accountMetrics: AccountMetrics;
  bot: Bot;
  longMetrics: SideMetrics;
  shortMetrics: SideMetrics;
}

export interface TradeHistoryTableProps {
  trades: BotTrade[];
}

export interface AccountCardProps {
  bot: Bot;
  accountMetrics: AccountMetrics;
  totalPnl: number;
  totalPnlPct: number;
}

export interface PositionsCardProps {
  bot: Bot;
  longPosition: PositionWsEvent | null;
  shortPosition: PositionWsEvent | null;
  longMetrics: SideMetrics;
  shortMetrics: SideMetrics;
  latestPrice: number;
}

export interface ChartMarker {
  time: number;
  position: 'aboveBar' | 'belowBar' | 'inBar';
  color: string;
  shape: 'circle' | 'square' | 'arrowUp' | 'arrowDown';
  text?: string;
}

export interface ChartAreaProps {
  klineData: KlineCandle[];
  timeframe: string;
  onTimeframeChange: (tf: string) => void;
  chartRef: { current: KlineChartHandle | null };
  markers: ChartMarker[];
  latestPrice: number;
}

export interface DesktopBotProps {
  bot: Bot;
  strategy: StrategyDetail | null;
  stats: BotStats | null;
  latestPrice: number;
  marketSummary: { changePct: number; high: number; low: number; volume: number };
  decision: AnalysisLog | null;
  logs: AnalysisLog[];
  trades: BotTrade[];
  accountMetrics: AccountMetrics;
  totalPnl: number;
  totalPnlPct: number;
  longPosition: PositionWsEvent | null;
  shortPosition: PositionWsEvent | null;
  longMetrics: SideMetrics;
  shortMetrics: SideMetrics;
  klineData: KlineCandle[];
  klineTimeframe: string;
  onTimeframeChange: (tf: string) => void;
  chartRef: { current: KlineChartHandle | null };
  markers: ChartMarker[];
}

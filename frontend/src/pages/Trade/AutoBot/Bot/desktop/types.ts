import type {
  AnalysisLog,
  AutoBot,
  AutoTrade,
  StrategyDetail,
  AutoBotStats,
  KlineCandle,
} from '@/service/types';
import type { PositionWsEvent } from '@/service/ws';
import type { KlineChartHandle } from '@/components/Chart/KlineChart';
import type { SideMetrics, AccountMetrics } from '../components/types';

export interface MarketHeaderProps {
  bot: AutoBot;
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
  bot: AutoBot;
  longMetrics: SideMetrics;
  shortMetrics: SideMetrics;
}

export interface TradeHistoryTableProps {
  trades: AutoTrade[];
}

export interface AccountCardProps {
  bot: AutoBot;
  accountMetrics: AccountMetrics;
  totalPnl: number;
  totalPnlPct: number;
}

export interface PositionsCardProps {
  bot: AutoBot;
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
  bot: AutoBot;
  strategy: StrategyDetail | null;
  stats: AutoBotStats | null;
  latestPrice: number;
  marketSummary: { changePct: number; high: number; low: number; volume: number };
  decision: AnalysisLog | null;
  logs: AnalysisLog[];
  trades: AutoTrade[];
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

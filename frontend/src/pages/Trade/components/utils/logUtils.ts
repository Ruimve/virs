import type { AnalysisLog } from '@/service';
import type { IndicatorSentiment } from '../IndicatorChip';
import type { RadarDataPoint } from '@/components/Radar';

export interface DecisionData {
  action: string;
  reason: string;
  confidence: number;
}

export const getDecision = (log: AnalysisLog | null): DecisionData | null => {
  if (!log?.result) return null;
  if (log.result.decision) return log.result.decision;
  if (log.result.action) {
    return {
      action: log.result.action,
      reason: log.result.reason ?? '',
      confidence: log.result.confidence ?? 0,
    };
  }
  return null;
};

export const toSentiment = (v: unknown): IndicatorSentiment => {
  if (typeof v === 'string') {
    if (v.includes('bull') || v.includes('buy') || v.includes('long')) return 'bullish';
    if (v.includes('bear') || v.includes('sell') || v.includes('short')) return 'bearish';
  }
  return 'neutral';
};

export interface IndicatorChipData {
  name: string;
  value: string;
  sentiment: IndicatorSentiment;
}

export const extractIndicatorChips = (
  market: Record<string, any> | undefined,
): IndicatorChipData[] => {
  if (!market) return [];

  const chips: IndicatorChipData[] = [];

  if (market.rsi != null) {
    chips.push({
      name: 'RSI',
      value: Number(market.rsi).toFixed(1),
      sentiment: toSentiment(market.rsi_signal),
    });
  }
  if (market.macd != null) {
    chips.push({
      name: 'MACD',
      value: Number(market.macd).toFixed(4),
      sentiment: toSentiment(market.macd_signal),
    });
  }
  if (market.ema != null) {
    chips.push({
      name: 'EMA',
      value: Number(market.ema).toFixed(2),
      sentiment: toSentiment(market.ema_signal),
    });
  }
  if (market.boll != null) {
    chips.push({
      name: 'BOLL',
      value: String(market.boll),
      sentiment: toSentiment(market.boll_signal),
    });
  }
  if (market.atr != null) {
    chips.push({ name: 'ATR', value: Number(market.atr).toFixed(2), sentiment: 'neutral' });
  }
  if (market.adx != null) {
    chips.push({
      name: 'ADX',
      value: Number(market.adx).toFixed(1),
      sentiment: toSentiment(market.adx_signal),
    });
  }

  return chips;
};

export const extractRadarData = (log: AnalysisLog | null): RadarDataPoint[] => {
  const market = log?.result?.market;
  const defaults: RadarDataPoint[] = [
    { label: '趋势', value: 0.5 },
    { label: '动量', value: 0.5 },
    { label: '波动', value: 0.5 },
    { label: '量能', value: 0.5 },
    { label: '情绪', value: 0.5 },
  ];
  if (!market || typeof market !== 'object') return defaults;

  const trend = typeof market.trend === 'number' ? market.trend : undefined;
  const momentum = typeof market.momentum === 'number' ? market.momentum : undefined;
  const volatility = typeof market.volatility === 'number' ? market.volatility : undefined;
  const volume = typeof market.volume === 'number' ? market.volume : undefined;
  const sentiment = typeof market.sentiment === 'number' ? market.sentiment : undefined;

  return [
    { label: '趋势', value: trend != null ? Math.min(Math.max(trend, 0), 1) : 0.5 },
    { label: '动量', value: momentum != null ? Math.min(Math.max(momentum, 0), 1) : 0.5 },
    { label: '波动', value: volatility != null ? Math.min(Math.max(volatility, 0), 1) : 0.5 },
    { label: '量能', value: volume != null ? Math.min(Math.max(volume, 0), 1) : 0.5 },
    { label: '情绪', value: sentiment != null ? Math.min(Math.max(sentiment, 0), 1) : 0.5 },
  ];
};

export interface TargetPrices {
  entry: number | null;
  stopLoss: number | null;
  takeProfit: number | null;
}

export const extractTargetPrices = (log: AnalysisLog | null): TargetPrices => {
  const risk = log?.result?.risk;
  const decision = log?.result?.decision;
  if (risk && typeof risk === 'object') {
    return {
      entry: typeof risk.entry_price === 'number' ? risk.entry_price : null,
      stopLoss: typeof risk.stop_loss === 'number' ? risk.stop_loss : null,
      takeProfit: typeof risk.take_profit === 'number' ? risk.take_profit : null,
    };
  }
  if (decision && typeof decision === 'object') {
    return {
      entry:
        typeof (decision as any).entry_price === 'number' ? (decision as any).entry_price : null,
      stopLoss:
        typeof (decision as any).stop_loss === 'number' ? (decision as any).stop_loss : null,
      takeProfit:
        typeof (decision as any).take_profit === 'number' ? (decision as any).take_profit : null,
    };
  }
  return { entry: null, stopLoss: null, takeProfit: null };
};

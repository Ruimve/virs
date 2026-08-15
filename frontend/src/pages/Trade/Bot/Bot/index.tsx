import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  fetchKlines,
  getBotAnalysisLogs,
  getBotTrades,
  getBotStats,
  type AnalysisLog,
  type Bot,
  type BotTrade,
  type BotStats,
  type KlineCandle,
} from '@/service';
import { useKlineWs, type KlineWsEvent } from '@/service/ws';
import type { KlineChartHandle } from '@/components/Chart/KlineChart';
import { useBot } from '@/pages/Trade/context/BotContext';
import { usePositionContext } from '../../context/PositionContext';
import { StickyMarket } from '../../components/StickyMarket';
import { UpperRegion } from './components/UpperRegion';
import { LowerRegion } from './components/LowerRegion';
import { DesktopBot } from './desktop/DesktopBot';
import { computeMetrics, computeAccount, computeTotalPnlPct } from './components/utils';

const useRafThrottledPrice = () => {
  const [latestPrice, setLatestPrice] = useState(0);
  const rafRef = useRef<number | undefined>(undefined);
  const pendingRef = useRef(0);

  const update = useCallback((price: number) => {
    pendingRef.current = price;
    if (rafRef.current != null) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = undefined;
      setLatestPrice((prev) => (pendingRef.current === prev ? prev : pendingRef.current));
    });
  }, []);

  useEffect(() => {
    return () => {
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  return { latestPrice, update };
};

function tradesToMarkers(trades: BotTrade[]) {
  const markers: Array<{
    time: number;
    position: 'belowBar' | 'aboveBar';
    color: string;
    shape: 'arrowUp' | 'arrowDown';
    text: string;
  }> = [];

  const fmtPrice = (p: number) => {
    if (!p) return '';
    return p >= 1000 ? `${(p / 1000).toFixed(1)}k` : p.toFixed(2);
  };

  const cs = getComputedStyle(document.documentElement);
  const upColor = cs.getPropertyValue('--chart-up').trim() || '#10b981';
  const downColor = cs.getPropertyValue('--chart-down').trim() || '#ef4444';

  for (const t of trades) {
    const openTime = Math.floor(new Date(t.opened_at).getTime() / 1000);
    const openIsBuy = t.open_side === 'buy';
    markers.push({
      time: openTime,
      position: openIsBuy ? 'belowBar' : 'aboveBar',
      color: openIsBuy ? upColor : downColor,
      shape: openIsBuy ? 'arrowUp' : 'arrowDown',
      text: fmtPrice(t.open_price),
    });

    if (t.status === 'closed' && t.closed_at && t.close_side) {
      const closeTime = Math.floor(new Date(t.closed_at).getTime() / 1000);
      const closeIsBuy = t.close_side === 'buy';
      markers.push({
        time: closeTime,
        position: closeIsBuy ? 'belowBar' : 'aboveBar',
        color: closeIsBuy ? upColor : downColor,
        shape: closeIsBuy ? 'arrowUp' : 'arrowDown',
        text: fmtPrice(t.close_price ?? 0),
      });
    }
  }

  return markers.sort((a, b) => a.time - b.time);
}

function computeMarketSummary(klineData: KlineCandle[], timeframe: string) {
  if (klineData.length === 0) return { changePct: 0, high: 0, low: 0, volume: 0 };

  const tfHours: Record<string, number> = {
    '1m': 1 / 60,
    '5m': 5 / 60,
    '15m': 15 / 60,
    '1h': 1,
    '4h': 4,
    '1d': 24,
  };
  const hoursPerCandle = tfHours[timeframe] ?? 1;
  const candlesIn24h = Math.min(Math.ceil(24 / hoursPerCandle), klineData.length);
  const recent = klineData.slice(-candlesIn24h);

  const firstClose = recent[0].close;
  const lastClose = recent[recent.length - 1].close;
  const changePct = firstClose > 0 ? ((lastClose - firstClose) / firstClose) * 100 : 0;
  const high = Math.max(...recent.map((k) => k.high));
  const low = Math.min(...recent.map((k) => k.low));
  const volume = recent.reduce((sum, k) => sum + (k.volume || 0), 0);

  return { changePct, high, low, volume };
}

const Bot = () => {
  const { bot, strategy } = useBot();
  const { positions } = usePositionContext();

  const [klineTimeframe, setKlineTimeframe] = useState('15m');
  const [klineData, setKlineData] = useState<KlineCandle[]>([]);
  const { latestPrice, update: updateLatestPrice } = useRafThrottledPrice();
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [botTrades, setBotTrades] = useState<BotTrade[]>([]);
  const [stats, setStats] = useState<BotStats | null>(null);

  const chartRef = useRef<KlineChartHandle>(null);

  const loadLogs = useCallback(async (botId: string) => {
    try {
      const res = await getBotAnalysisLogs({ botId, page: 1, pageSize: 1 });
      if (res.data?.items) setLogs(res.data.items);
    } catch (e) {
      console.error('Failed to load analysis logs:', e);
    }
  }, []);

  const loadTrades = useCallback(async (botId: string) => {
    try {
      const res = await getBotTrades({ botId, page: 1, pageSize: 50 });
      if (res.data?.trades) setBotTrades(res.data.trades);
    } catch (e) {
      console.error('Failed to load trades:', e);
    }
  }, []);

  const loadStats = useCallback(async (botId: string) => {
    try {
      const res = await getBotStats({ botId });
      if (res.success && res.data) setStats(res.data);
    } catch (e) {
      console.error('Failed to load stats:', e);
    }
  }, []);

  const loadKlines = useCallback(async (exchange: string, symbol: string, tf: string) => {
    try {
      const res = await fetchKlines({ exchange, symbol, timeframe: tf });
      if (res.data) setKlineData(res.data);
    } catch (e) {
      console.error('Failed to load kline:', e);
    }
  }, []);

  const loadKlineStable = useCallback(() => {
    if (!bot?.exchange || !bot?.symbol || !klineTimeframe) return;
    loadKlines(bot.exchange, bot.symbol, klineTimeframe);
  }, [bot?.exchange, bot?.symbol, klineTimeframe, loadKlines]);

  useEffect(() => {
    if (!bot?.id) return;
    loadLogs(bot.id);
    loadTrades(bot.id);
    loadStats(bot.id);
  }, [bot?.id, loadLogs, loadTrades, loadStats]);

  useEffect(() => {
    if (!bot?.exchange || !bot?.symbol || !klineTimeframe) return;
    loadKlines(bot.exchange, bot.symbol, klineTimeframe);
  }, [bot?.exchange, bot?.symbol, klineTimeframe, loadKlines]);

  useKlineWs(
    (event: KlineWsEvent) => {
      if (!bot) return;
      if (event.symbol !== bot.symbol || event.exchange !== bot.exchange) return;
      const c = event.candle;
      if (!c) return;
      updateLatestPrice(c.close);
      chartRef.current?.update(c);
    },
    loadKlineStable,
    klineTimeframe,
  );

  const botData = useMemo(() => bot as Bot, [bot]);
  const markers = useMemo(() => tradesToMarkers(botTrades), [botTrades]);
  const latestDecision = useMemo(() => logs[0] || null, [logs]);

  const longMetrics = useMemo(
    () => computeMetrics(positions.long, latestPrice, botData.leverage),
    [positions.long, latestPrice, botData.leverage],
  );
  const shortMetrics = useMemo(
    () => computeMetrics(positions.short, latestPrice, botData.leverage),
    [positions.short, latestPrice, botData.leverage],
  );

  const totalUnrealizedPnl = longMetrics.unrealizedPnl + shortMetrics.unrealizedPnl;
  const totalUsedMargin = longMetrics.usedMargin + shortMetrics.usedMargin;

  const accountMetrics = useMemo(
    () => computeAccount(botData, totalUnrealizedPnl, totalUsedMargin),
    [botData, totalUnrealizedPnl, totalUsedMargin],
  );

  const totalPnl = botData.total_pnl + totalUnrealizedPnl;
  const totalPnlPct = computeTotalPnlPct(botData, totalPnl);

  const marketSummary = useMemo(
    () => computeMarketSummary(klineData, klineTimeframe),
    [klineData, klineTimeframe],
  );

  return (
    <>
      <title>交易 - VIRS</title>
      {/* 桌面端布局：侧边栏 + 顶栏 + 双列主区域 */}
      <DesktopBot
        bot={botData}
        strategy={strategy}
        stats={stats}
        latestPrice={latestPrice}
        marketSummary={marketSummary}
        decision={latestDecision}
        logs={logs}
        trades={botTrades}
        accountMetrics={accountMetrics}
        totalPnl={totalPnl}
        totalPnlPct={totalPnlPct}
        longPosition={positions.long}
        shortPosition={positions.short}
        longMetrics={longMetrics}
        shortMetrics={shortMetrics}
        klineData={klineData}
        klineTimeframe={klineTimeframe}
        onTimeframeChange={setKlineTimeframe}
        chartRef={chartRef}
        markers={markers}
      />

      {/* 移动端布局：三段式（UpperRegion → LowerRegion → StickyMarket） */}
      <div className="md:hidden h-full flex flex-col">
        <UpperRegion
          bot={botData}
          strategy={strategy}
          latestPrice={latestPrice}
          marketSummary={marketSummary}
          decision={latestDecision}
          stats={stats}
          accountMetrics={accountMetrics}
          totalPnl={totalPnl}
          totalPnlPct={totalPnlPct}
        />

        <LowerRegion
          bot={botData}
          latestPrice={latestPrice}
          longPosition={positions.long}
          shortPosition={positions.short}
          longMetrics={longMetrics}
          shortMetrics={shortMetrics}
          decision={latestDecision}
        />

        <StickyMarket
          klineData={klineData}
          klineTimeframe={klineTimeframe}
          onTimeframeChange={setKlineTimeframe}
          chartRef={chartRef}
          markers={markers}
          latestPrice={latestPrice}
          logs={logs}
        />
      </div>
    </>
  );
};

export default Bot;

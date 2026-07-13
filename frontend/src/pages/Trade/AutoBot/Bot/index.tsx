import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  fetchKlines,
  getAutoAnalysisLogs,
  getAutoTrades,
  type AnalysisLog,
  type AutoBot,
  type AutoTrade,
  type KlineCandle,
} from '@/service';
import { useKlineWs, type KlineWsEvent } from '@/service/ws';
import type { KlineChartHandle } from '@/components/Chart/KlineChart';
import { useBot } from '../../context/BotContext';
import DecisionCard from '../../components/DecisionCard';
import TradeStats from './TradeStats';
import StickyMarket from '../../components/StickyMarket';
import PositionStats from './PositionStats';
import RecentDecisions from './RecentDecisions';
import RecentTrades from './RecentTrades';

/**
 * WS price update rAF throttle + price change check.
 *
 * Merges multiple price pushes within the same animation frame into a single setState,
 * and only triggers re-render when the price actually changes.
 * Avoids high-frequency WS heartbeat causing Bot full-tree reconciliation.
 */
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

/**
 * Convert trade records to Kline chart markers.
 * Open (open_side=buy) -> green up arrow, below bar
 * Open (open_side=sell) -> red down arrow, above bar
 * Close (close_side=buy) -> green up arrow
 * Close (close_side=sell) -> red down arrow
 */
function tradesToMarkers(trades: AutoTrade[]) {
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

const Bot = () => {
  const { bot } = useBot();

  const [klineTimeframe, setKlineTimeframe] = useState('15m');
  const [klineData, setKlineData] = useState<KlineCandle[]>([]);
  const { latestPrice, update: updateLatestPrice } = useRafThrottledPrice();
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [autoTrades, setAutoTrades] = useState<AutoTrade[]>([]);

  const chartRef = useRef<KlineChartHandle>(null);

  const loadLogs = useCallback(async (botId: string) => {
    try {
      const res = await getAutoAnalysisLogs(botId, 1, 1);
      if (res.data?.items) setLogs(res.data.items);
    } catch (e) {
      console.error('Failed to load analysis logs:', e);
    }
  }, []);

  const loadTrades = useCallback(async (botId: string) => {
    try {
      const res = await getAutoTrades(botId, 1, 50);
      if (res.data?.trades) setAutoTrades(res.data.trades);
    } catch (e) {
      console.error('Failed to load trades:', e);
    }
  }, []);

  const loadKlines = useCallback(async (exchange: string, symbol: string, tf: string) => {
    try {
      const res = await fetchKlines({
        exchange,
        symbol,
        timeframe: tf,
      });
      if (res.data) setKlineData(res.data);
    } catch (e) {
      console.error('Failed to load kline:', e);
    }
  }, []);

  const loadKlineStable = useCallback(() => {
    if (!bot?.exchange || !bot?.symbol || !klineTimeframe) return;
    loadKlines(bot?.exchange, bot?.symbol, klineTimeframe);
  }, [bot?.exchange, bot?.symbol, klineTimeframe, loadKlines]);

  useEffect(() => {
    if (!bot?.id) return;
    loadLogs(bot?.id);
    loadTrades(bot?.id);
  }, [bot?.id, loadLogs, loadTrades]);

  useEffect(() => {
    if (!bot?.exchange || !bot?.symbol || !klineTimeframe) return;
    loadKlines(bot?.exchange, bot?.symbol, klineTimeframe);
  }, [bot?.exchange, bot?.symbol, klineTimeframe, loadKlines]);

  useKlineWs(
    (event: KlineWsEvent) => {
      if (!bot) return;
      if (event.symbol !== bot?.symbol || event.exchange !== bot?.exchange) return;
      const c = event.candle;
      if (!c) return;
      updateLatestPrice(c.close);
      chartRef.current?.update(c);
    },
    loadKlineStable,
    klineTimeframe,
  );

  const autoBot = useMemo(() => bot as AutoBot, [bot]);

  const markers = useMemo(() => tradesToMarkers(autoTrades), [autoTrades]);
  const latestDecision = useMemo(() => logs[0] || null, [logs]);

  return (
    <div className="h-full flex flex-col lg:flex-row">
      {/* Main area: position stats + AI decision + trade stats + bottom market panel */}
      <div className="flex flex-col h-full lg:flex-1 lg:min-h-0 overflow-y-auto relative mb-9">
        {/* Position stats */}
        <PositionStats bot={autoBot} latestPrice={latestPrice} />

        {/* AI decision card */}
        <DecisionCard log={latestDecision} botId={autoBot?.id} botType="auto" />

        {/* Historical trade stats */}
        <TradeStats botId={autoBot?.id} />

        {/* Bottom market collapsible panel (Kline chart) */}
        <div className="fixed bottom-0 left-0 right-0">
          <StickyMarket
            klineData={klineData}
            klineTimeframe={klineTimeframe}
            onTimeframeChange={setKlineTimeframe}
            chartRef={chartRef}
            markers={markers}
            latestPrice={latestPrice}
          />
        </div>
      </div>

      {/* Right sidebar: recent decisions + recent trades */}
      <div className="hidden lg:flex w-72 xl:w-80 border-l border-line-subtle/50 flex-col bg-surface-1/20">
        <div className="flex flex-col h-full divide-y divide-line-subtle/50">
          <div className="flex-1 min-h-0">
            <RecentDecisions logs={logs} botId={autoBot?.id} botType={'auto'} />
          </div>
          <div className="flex-1 min-h-0">
            <RecentTrades trades={autoTrades} />
          </div>
        </div>
      </div>
    </div>
  );
};

export default Bot;

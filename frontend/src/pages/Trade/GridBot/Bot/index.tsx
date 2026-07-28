import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  fetchKlines,
  getGridAnalysisLogs,
  getGridTrades,
  type AnalysisLog,
  type GridBot,
  type GridTrade,
  type KlineCandle,
  type KlineWsEvent,
} from '@/service';
import { useKlineWs } from '@/service/ws';
import { type KlineChartHandle } from '@/components/Chart/KlineChart';
import { useBot } from '../../context/BotContext';
import { DecisionCard } from '../../components/DecisionCard';
import { StickyMarket } from '../../components/StickyMarket';
import { TradeStats } from './TradeStats';
import { LevelsOverview } from './LevelsOverview';
import { RecentTrades } from './RecentTrades';
import { PositionStats } from './PositionStats';

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

function tradesToMarkers(trades: GridTrade[]) {
  const cs = getComputedStyle(document.documentElement);
  const upColor = cs.getPropertyValue('--chart-up').trim() || '#10b981';
  const downColor = cs.getPropertyValue('--chart-down').trim() || '#ef4444';
  return trades
    .map((t) => {
      const time = Math.floor(new Date(t.opened_at).getTime() / 1000);
      const isBuy = t.open_side === 'buy';
      return {
        time,
        position: isBuy ? ('belowBar' as const) : ('aboveBar' as const),
        color: isBuy ? upColor : downColor,
        shape: isBuy ? ('arrowUp' as const) : ('arrowDown' as const),
        text: `${isBuy ? '买' : '卖'} L${t.grid_level}`,
      };
    })
    .sort((a, b) => a.time - b.time);
}

const Bot = () => {
  const { bot, gridLevels } = useBot();

  const [klineTimeframe, setKlineTimeframe] = useState('15m');
  const [klineData, setKlineData] = useState<KlineCandle[]>([]);
  const { latestPrice, update: updateLatestPrice } = useRafThrottledPrice();
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [gridTrades, setGridTrades] = useState<GridTrade[]>([]);

  const chartRef = useRef<KlineChartHandle>(null);

  const loadLogs = useCallback(async (botId: string) => {
    try {
      const res = await getGridAnalysisLogs(botId, 1, 1);
      if (res.data?.items) setLogs(res.data.items);
    } catch (e) {
      console.error('Failed to load analysis logs:', e);
    }
  }, []);

  const loadTrades = useCallback(async (botId: string) => {
    try {
      const res = await getGridTrades(botId, 1, 50);
      if (res.data?.trades) setGridTrades(res.data.trades);
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

  const gridBot = useMemo(() => bot as GridBot, [bot]);
  const markers = useMemo(() => tradesToMarkers(gridTrades), [gridTrades]);
  const latestDecision = useMemo(() => logs[0] || null, [logs]);

  return (
    <div className="h-full flex flex-col lg:flex-row">
      {}
      <div className="flex flex-col h-full lg:flex-1 lg:min-h-0 overflow-y-auto relative mb-9">
        {}
        <PositionStats bot={gridBot} latestPrice={latestPrice} />

        {}
        <DecisionCard log={latestDecision} botId={gridBot?.id} botType="grid" />

        {}
        <TradeStats botId={gridBot?.id} />

        {}
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

      {}
      <div className="hidden lg:flex w-72 xl:w-80 border-l border-line-subtle flex-col">
        <div className="flex flex-col h-full divide-y divide-line-subtle">
          <div className="flex-1 min-h-0">
            <LevelsOverview gridLevels={gridLevels} />
          </div>
          <div className="flex-1 min-h-0">
            <RecentTrades botId={gridBot?.id} />
          </div>
        </div>
      </div>
    </div>
  );
};

export default Bot;

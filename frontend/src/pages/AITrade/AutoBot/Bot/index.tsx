import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
import AIDecisionCard from '../../components/AIDecisionCard';
import TradeStats from './TradeStats';
import StickyMarket from '../../components/StickyMarket';
import PositionStats from './PositionStats';
import RecentDecisions from './RecentDecisions';
import RecentTrades from './RecentTrades';

/**
 * 把交易记录转换为 K线图 markers。
 * 开仓（open_side=buy）→ 绿色向上箭头，位于 K线下方
 * 开仓（open_side=sell）→ 红色向下箭头，位于 K线上方
 * 平仓（close_side=buy）→ 绿色向上箭头
 * 平仓（close_side=sell）→ 红色向下箭头
 */
function tradesToMarkers(trades: AutoTrade[]) {
  const markers: Array<{
    time: number;
    position: 'belowBar' | 'aboveBar';
    color: string;
    shape: 'arrowUp' | 'arrowDown';
    text: string;
  }> = [];

  for (const t of trades) {
    // 开仓 marker
    const openTime = Math.floor(new Date(t.opened_at).getTime() / 1000);
    const openIsBuy = t.open_side === 'buy';
    markers.push({
      time: openTime,
      position: openIsBuy ? 'belowBar' : 'aboveBar',
      color: openIsBuy ? '#10b981' : '#ef4444',
      shape: openIsBuy ? 'arrowUp' : 'arrowDown',
      text: `${openIsBuy ? '开多' : '开空'} ${t.open_price.toFixed(2)}`,
    });

    // 平仓 marker（仅已平仓记录）
    if (t.status === 'closed' && t.closed_at && t.close_side) {
      const closeTime = Math.floor(new Date(t.closed_at).getTime() / 1000);
      const closeIsBuy = t.close_side === 'buy';
      markers.push({
        time: closeTime,
        position: closeIsBuy ? 'belowBar' : 'aboveBar',
        color: closeIsBuy ? '#10b981' : '#ef4444',
        shape: closeIsBuy ? 'arrowUp' : 'arrowDown',
        text: `${closeIsBuy ? '平空' : '平多'} ${t.close_price?.toFixed(2)}`,
      });
    }
  }

  return markers.sort((a, b) => a.time - b.time);
}

const Bot = () => {
  const { bot } = useBot();

  const [klineTimeframe, setKlineTimeframe] = useState('15m');
  const [klineData, setKlineData] = useState<KlineCandle[]>([]);
  const [latestPrice, setLatestPrice] = useState(0);
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [autoTrades, setAutoTrades] = useState<AutoTrade[]>([]);

  const chartRef = useRef<KlineChartHandle>(null);

  const loadLogs = useCallback(async (botId: string) => {
    try {
      const res = await getAutoAnalysisLogs(botId);
      if (res.data?.logs) setLogs(res.data.logs);
    } catch (e) {
      console.error('Failed to load analysis logs:', e);
    }
  }, []);

  const loadTrades = useCallback(async (botId: string) => {
    try {
      // 获取最近 50 条用于 K 线 markers
      const res = await getAutoTrades(botId, 1, 50);
      if (res.data?.trades) setAutoTrades(res.data.trades);
    } catch (e) {
      console.error('Failed to load trades:', e);
    }
  }, []);

  const loadKlines = useCallback(
    async (exchange: string, symbol: string, market_type: string, tf: string) => {
      try {
        const res = await fetchKlines({
          exchange,
          symbol,
          market_type,
          timeframe: tf,
        });
        if (res.data) setKlineData(res.data);
      } catch (e) {
        console.error('Failed to load kline:', e);
      }
    },
    [],
  );

  const loadKlineStable = useCallback(() => {
    if (!bot?.exchange || !bot?.symbol || !bot?.market_type || !klineTimeframe) return;
    loadKlines(bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe);
  }, [bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe, loadKlines]);

  useEffect(() => {
    if (!bot?.id) return;
    loadLogs(bot?.id);
    loadTrades(bot?.id);
  }, [bot?.id, loadLogs, loadTrades]);

  useEffect(() => {
    if (!bot?.exchange || !bot?.symbol || !bot?.market_type || !klineTimeframe) return;
    loadKlines(bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe);
  }, [bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe, loadKlines]);

  useKlineWs(
    (event: KlineWsEvent) => {
      if (!bot) return;
      if (event.symbol !== bot?.symbol || event.exchange !== bot?.exchange) return;
      const c = event.candle;
      if (!c) return;
      // 更新最新价
      setLatestPrice(c.close);
      // Update chart directly via series.update() — no re-render
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
      {/* 主区域：状态栏 + AI决策 + 交易统计 + 底部行情折叠 */}
      <div className="flex flex-col h-full lg:flex-1 lg:min-h-0 overflow-y-auto relative mb-9">
        {/* 仓位状态 */}
        <PositionStats bot={autoBot} latestPrice={latestPrice} />

        {/* AI 决策卡片 */}
        <AIDecisionCard log={latestDecision} botId={autoBot?.id} botType="auto" />

        {/* 历史交易统计 */}
        <TradeStats botId={autoBot?.id} />

        {/* 底部行情折叠面板（K线图） */}
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

      {/* 右侧侧边栏：最近决策 + 最近成交 */}
      <div className="hidden lg:flex w-72 xl:w-80 border-l border-line-subtle flex-col">
        <div className="flex flex-col h-full divide-y divide-line-subtle">
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

export default memo(Bot);

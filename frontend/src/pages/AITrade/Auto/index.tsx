import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { getAutoBotDetail, getAutoAnalysisLogs, startAutoBot, stopAutoBot, deleteAutoBot } from '../../../service/bot';
import { fetchKlines, fetchOrderBook, subscribeOrderBook } from '../../../service/market';
import type { AutoBot, AutoTrade, AnalysisLog, KlineCandle, OrderBookData } from '../../../service/types';
import { useKlineWs, useOrderBookWs, type KlineWsEvent, type OrderBookWsEvent } from '../../../service/ws';
import BotDetailHeader from '../components/BotDetailHeader';
import ChartPanel from '../components/ChartPanel';
import type { KlineChartHandle } from '../../../components/Chart/KlineChart';
import OrderBookPanel from '../components/OrderBookPanel';
import AnalysisList from '../components/AnalysisList';
import PositionStats from './PositionStats';
import MobileOrderBook from '../components/MobileOrderBook';
import MarketIndicators from '../components/MarketIndicators';
import type { TabConfig } from '../components/shared';
import { formatPnl } from '../components/shared';

type Tab = 'market' | 'trades' | 'analysis';

const tabs: TabConfig[] = [
  { key: 'market', label: '行情' },
  { key: 'trades', label: '交易记录' },
  { key: 'analysis', label: 'AI 决策日志' },
];

// ── Helpers ───────────────────────────────────────────────

const tradeTypeLabel = (t: string) => {
  const map: Record<string, string> = {
    open_long: '开多', open_short: '开空',
    close_long: '平多', close_short: '平空',
    stop_loss: '止损', take_profit: '止盈',
  };
  return map[t] || t;
};

const tradeTypeColor = (t: string) => {
  if (t.startsWith('open_long') || t === 'close_short') return 'text-emerald-400';
  if (t.startsWith('open_short') || t === 'close_long') return 'text-red-400';
  if (t === 'stop_loss') return 'text-red-400';
  if (t === 'take_profit') return 'text-emerald-400';
  return 'text-on-surface-tertiary';
};



// ── Page ──────────────────────────────────────────────────

export default function AutoDetailPage() {
  const params = useParams();
  const navigate = useNavigate();

  const activeTab: Tab = (params.tab as Tab) || 'market';

  const [bot, setBot] = useState<AutoBot | null>(null);
  const [trades, setTrades] = useState<AutoTrade[]>([]);
  const [logs, setLogs] = useState<AnalysisLog[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  // Market data
  const [klineData, setKlineData] = useState<KlineCandle[]>([]);
  const [klineTimeframe, setKlineTimeframe] = useState('15m');
  const [orderBook, setOrderBook] = useState<OrderBookData>({ bids: [], asks: [] });
  const chartRef = useRef<KlineChartHandle>(null);

  const botRef = useRef(bot);
  botRef.current = bot;

  // ── Data loading ──────────────────────────────────────

  const loadBot = useCallback(async () => {
    try {
      const res = await getAutoBotDetail(params.id!);
      if (res.data?.bot) {
        setBot(res.data.bot);
        setTrades(res.data.trades || []);
      } else {
        setError('机器人不存在或加载失败');
      }
    } catch (e) {
      console.error('Failed to load auto bot:', e);
      setError('加载失败');
    } finally {
      setLoading(false);
    }
  }, [params.id]);

  const loadLogs = useCallback(async () => {
    try {
      const res = await getAutoAnalysisLogs(params.id!);
      if (res.data?.logs) setLogs(res.data.logs);
    } catch (e) {
      console.error('Failed to load analysis logs:', e);
    }
  }, [params.id]);

  const loadKlines = useCallback(async () => {
    const b = botRef.current;
    if (!b) return;
    try {
      const res = await fetchKlines({ exchange: b.exchange, symbol: b.symbol, market_type: b.market_type, timeframe: klineTimeframe });
      if (res.data) setKlineData(res.data);
    } catch (e) {
      console.error('Failed to load kline:', e);
    }
  }, [klineTimeframe]);

  const loadOrderBook = useCallback(async () => {
    const b = botRef.current;
    if (!b) return;
    try {
      const mt = b.market_type || 'perpetual';
      // 确保后端订阅了该 symbol 的订单簿流（后端重启后订阅会丢失）
      await subscribeOrderBook({ exchange: b.exchange, symbol: b.symbol, market_type: mt });
      const res = await fetchOrderBook({ exchange: b.exchange, symbol: b.symbol, market_type: mt });
      if (res.data) setOrderBook(res.data);
    } catch (e) {
      console.error('Failed to load orderbook:', e);
    }
  }, []);

  // ── Actions ───────────────────────────────────────────

  const handleAction = async (action: 'start' | 'stop' | 'delete') => {
    try {
      if (action === 'delete') {
        if (!confirm('确定删除此机器人？将平仓所有持仓。')) return;
        await deleteAutoBot(params.id!);
        navigate('/setup/bot-type', { replace: true });
        return;
      }
      if (action === 'start') await startAutoBot(params.id!);
      else if (action === 'stop') await stopAutoBot(params.id!);
      await loadBot();
    } catch (e: any) {
      setError(e.response?.data?.error || '操作失败');
    }
  };

  const handleTabChange = (key: string) => {
    if (key === 'market') {
      navigate(`/trade/auto/${params.id}`, { replace: true });
    } else {
      navigate(`/trade/auto/${params.id}/${key}`, { replace: true });
    }
  };

  // ── Effects ───────────────────────────────────────────

  useEffect(() => {
    loadBot();
    loadLogs();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (bot) { loadKlines(); loadOrderBook(); }
  }, [bot, klineTimeframe]); // eslint-disable-line react-hooks/exhaustive-deps

  // Real-time orderbook via WebSocket (replaces 2s polling)
  useOrderBookWs(
    (event: OrderBookWsEvent) => {
      const b = botRef.current;
      if (!b || event.symbol !== b.symbol || event.exchange !== b.exchange) return;
      setOrderBook(event.orderBook);
    },
  );

  useKlineWs(
    (event: KlineWsEvent) => {
      const b = botRef.current;
      if (!b || event.symbol !== b.symbol || event.exchange !== b.exchange || klineTimeframe !== event.timeframe) return;
      const c = event.candle;
      if (!c) return;
      // Update chart directly via series.update() — no re-render
      chartRef.current?.update(c);
    },
    loadKlines,
  );

  // ── Render ────────────────────────────────────────────

  if (loading || !bot) {
    return (
      <div className="h-screen bg-base flex flex-col items-center justify-center relative gap-4">
        {!loading && !bot ? (
          <>
            <div className="text-on-surface-tertiary text-sm">{error || '机器人不存在或加载失败'}</div>
            <button
              onClick={() => navigate('/setup/bot-type', { replace: true })}
              className="px-4 py-2 text-xs font-medium border border-line-default rounded-lg text-on-surface-tertiary hover:bg-surface-2 transition-colors"
            >
              创建新机器人
            </button>
          </>
        ) : (
          <svg className="animate-spin h-6 w-6 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
        )}
      </div>
    );
  }

  const b = bot;

  return (
    <div className="h-screen bg-base flex flex-col relative overflow-hidden">
      {/* Background glow */}
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-indigo-500/[0.03] blur-[120px]" />
      </div>

      {/* Header */}
      <BotDetailHeader
        bot={{ id: b.id, name: b.name, symbol: b.symbol, exchange: b.exchange, market_type: b.market_type, status: b.status, leverage: b.leverage }}
        tabs={tabs}
        activeTab={activeTab}
        onTabChange={handleTabChange}
        actions={{
          onStop: () => handleAction('stop'),
          onStart: () => handleAction('start'),
          onDelete: () => handleAction('delete'),
        }}
        showStartWhenStopped={true}
      />

      {/* Content */}
      <div className="flex-1 overflow-y-auto relative z-10">
        {error && (
          <div className="px-4 md:px-8 pt-4">
            <div className="text-xs text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2">{error}</div>
          </div>
        )}

        {/* Market tab */}
        {activeTab === 'market' && (
          <div className="h-full flex flex-col lg:flex-row">
            <div className="flex flex-col h-full lg:flex-1 lg:min-h-0">
              <PositionStats bot={b} />
              <div className="h-[260px] shrink-0 lg:h-auto lg:flex-1 lg:min-h-0 lg:shrink">
                <ChartPanel
                  klineData={klineData}
                  klineTimeframe={klineTimeframe}
                  onTimeframeChange={setKlineTimeframe}
                  chartRef={chartRef}
                />
              </div>
              {/* Mobile: market indicators + recent trades (scrollable) */}
              <div className="flex-1 overflow-y-auto lg:hidden border-t border-line-subtle">
                <MarketIndicators klineData={klineData} orderBook={orderBook} />
                {trades.length > 0 && (
                  <div className="px-4 py-2">
                    <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">最近成交</div>
                    {trades.slice(0, 8).map((t) => (
                      <div key={t.id} className="flex items-center justify-between py-1 text-xs">
                        <span className={t.side === 'buy' ? 'text-emerald-400' : 'text-red-400'}>
                          {t.side === 'buy' ? '买' : '卖'} {t.quantity.toFixed(4)} @ {t.price.toFixed(2)}
                        </span>
                        <span className="text-on-surface-tertiary text-[10px]">
                          {new Date(t.created_at).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
              {/* Mobile orderbook — pinned at bottom */}
              <div className="shrink-0 lg:hidden border-t border-line-subtle">
                <MobileOrderBook orderBook={orderBook} />
              </div>
            </div>
            <div className="hidden lg:flex w-72 xl:w-80 border-l border-line-subtle flex-col">
              <OrderBookPanel orderBook={orderBook} />
            </div>
          </div>
        )}

        {/* Trades tab */}
        {activeTab === 'trades' && (
          <div className="max-w-5xl mx-auto px-4 md:px-8 py-6">
            {trades.length > 0 ? (
              <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
                <div className="divide-y divide-line-subtle">
                  {trades.slice(0, 50).map((t) => (
                    <div key={t.id} className="flex items-center justify-between px-5 py-3 hover:bg-surface-2/50">
                      <div className="flex items-center gap-3">
                        <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${tradeTypeColor(t.trade_type)} bg-surface-2`}>
                          {tradeTypeLabel(t.trade_type)}
                        </span>
                        <div>
                          <div className="text-xs text-on-surface font-mono">
                            {t.side === 'buy' ? '买入' : '卖出'} {t.quantity.toFixed(6)} @ {t.price.toFixed(2)}
                          </div>
                          <div className="text-[10px] text-on-surface-tertiary mt-0.5">
                            {new Date(t.created_at).toLocaleString('zh-CN')}
                          </div>
                        </div>
                      </div>
                      {t.pnl !== 0 && (
                        <div className="text-right">
                          {formatPnl(t.pnl)}
                          {t.pnl_pct !== 0 && <div className="text-[10px] text-on-surface-tertiary">{t.pnl_pct.toFixed(2)}%</div>}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="text-center py-12 text-on-surface-tertiary text-xs">暂无交易记录</div>
            )}
          </div>
        )}

        {/* Analysis tab */}
        {activeTab === 'analysis' && (
          <AnalysisList logs={logs} loading={false} botType="auto" botId={params.id!} />
        )}
      </div>
    </div>
  );
}

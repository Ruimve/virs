import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { getGridBotDetail, getGridTrades, getGridAnalysisLogs, startGridBot, stopGridBot, deleteGridBot } from '../../../service/bot';
import { fetchKlines, fetchOrderBook, subscribeOrderBook } from '../../../service/market';
import type { GridBot, GridLevelInfo, GridTrade, AnalysisLog, KlineCandle, OrderBookData } from '../../../service/types';
import { useKlineWs, useOrderBookWs, type KlineWsEvent, type OrderBookWsEvent } from '../../../service/ws';
import BotDetailHeader from '../components/BotDetailHeader';
import ChartPanel from '../components/ChartPanel';
import type { KlineChartHandle } from '../../../components/Chart/KlineChart';
import OrderBookPanel from '../components/OrderBookPanel';
import AnalysisList from '../components/AnalysisList';
import GridStats from './GridStats';
import MobileOrderBook from '../components/MobileOrderBook';
import MarketIndicators from '../components/MarketIndicators';
import GridLevelsTab from './GridLevelsTab';
import type { TabConfig } from '../components/shared';
import { formatPnlShort } from '../components/shared';

type Tab = 'market' | 'levels' | 'trades' | 'analysis';

const tabs: TabConfig[] = [
  { key: 'market', label: '行情' },
  { key: 'levels', label: '网格层级' },
  { key: 'trades', label: '交易记录' },
  { key: 'analysis', label: 'AI 决策日志' },
];

// ── Page ──────────────────────────────────────────────────

export default function GridDetailPage() {
  const params = useParams();
  const navigate = useNavigate();

  const activeTab: Tab = (params.tab as Tab) || 'market';

  const [bot, setBot] = useState<GridBot | null>(null);
  const [error, setError] = useState('');
  const [gridLevels, setGridLevels] = useState<GridLevelInfo[]>([]);
  const [trades, setTrades] = useState<GridTrade[]>([]);
  const [loadingTrades, setLoadingTrades] = useState(false);
  const [analysisLogs, setAnalysisLogs] = useState<AnalysisLog[]>([]);
  const [loadingAnalysis, setLoadingAnalysis] = useState(false);

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
      const res = await getGridBotDetail(params.id!);
      if (res.data?.bot) {
        setBot(res.data.bot);
        if (res.data.grid_levels?.length) setGridLevels(res.data.grid_levels);
        if (res.data.trades?.length) setTrades(res.data.trades);
      } else {
        setError('机器人不存在');
      }
    } catch (e) {
      setError('加载失败');
    }
  }, [params.id]);

  const loadTrades = useCallback(async () => {
    setLoadingTrades(true);
    try {
      const res = await getGridTrades(params.id!);
      setTrades(res.data?.items || []);
      setGridLevels(res.data?.grid_levels || []);
    } catch (e) {
      console.error(e);
    } finally {
      setLoadingTrades(false);
    }
  }, [params.id]);

  const loadAnalysis = useCallback(async () => {
    setLoadingAnalysis(true);
    try {
      const res = await getGridAnalysisLogs(params.id!);
      setAnalysisLogs(res.data?.items || []);
    } catch (e) {
      console.error(e);
    } finally {
      setLoadingAnalysis(false);
    }
  }, [params.id]);

  const loadKline = useCallback(async () => {
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
      if (action === 'start') await startGridBot(params.id!);
      else if (action === 'stop') await stopGridBot(params.id!);
      else if (action === 'delete') {
        if (!confirm('确定删除此机器人？')) return;
        await stopGridBot(params.id!).catch(() => {});
        await deleteGridBot(params.id!);
        navigate('/setup/bot-type', { replace: true });
        return;
      }
      await loadBot();
    } catch (e: any) {
      setError(e.response?.data?.error || '操作失败');
    }
  };

  // ── Effects ───────────────────────────────────────────

  useKlineWs(
    (event: KlineWsEvent) => {
      const b = botRef.current;
      if (!b || event.symbol !== b.symbol || event.exchange !== b.exchange) return;
      const c = event.candle;
      if (!c) return;
      // Update chart directly via series.update() — no re-render
      chartRef.current?.update(c);
    },
    loadKline,
  );

  useEffect(() => {
    loadBot();
    loadTrades();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (bot) { loadKline(); loadOrderBook(); }
  }, [bot, klineTimeframe]); // eslint-disable-line react-hooks/exhaustive-deps

  // Real-time orderbook via WebSocket (replaces 2s polling)
  useOrderBookWs(
    (event: OrderBookWsEvent) => {
      const b = botRef.current;
      if (!b || event.symbol !== b.symbol || event.exchange !== b.exchange) return;
      setOrderBook(event.orderBook);
    },
  );

  const handleTabChange = (key: string) => {
    if (key === 'analysis' && analysisLogs.length === 0) loadAnalysis();
    if (key === 'market') {
      navigate(`/trade/grid/${params.id}`, { replace: true });
    } else {
      navigate(`/trade/grid/${params.id}/${key}`, { replace: true });
    }
  };

  // ── Render ────────────────────────────────────────────

  if (!bot) {
    return (
      <div className="flex-1 flex items-center justify-center relative z-10 h-screen bg-base">
        {error ? (
          <div className="text-red-400 text-sm">{error}</div>
        ) : (
          <svg className="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
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
        showStartWhenStopped={false}
        pulseOnRunning={true}
      />

      {/* Content */}
      <div className="flex-1 overflow-y-auto relative z-10">
        {/* Market tab */}
        {activeTab === 'market' && (
          <div className="h-full flex flex-col lg:flex-row">
            <div className="flex flex-col h-full lg:flex-1 lg:min-h-0">
              <GridStats bot={b} />
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
                    {trades.slice(0, 5).map((t, i) => (
                      <div key={i} className="flex items-center justify-between py-1 text-xs">
                        <span className={t.open_side === 'buy' ? 'text-emerald-400' : 'text-red-400'}>
                          {t.open_side === 'buy' ? '买' : '卖'} {t.open_quantity?.toFixed(4) || '-'} @ {t.open_price?.toFixed(2) || '-'}
                        </span>
                        <span className="text-on-surface-tertiary text-[10px]">
                          {new Date(t.opened_at).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}
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

        {/* Levels tab */}
        {activeTab === 'levels' && (
          <GridLevelsTab gridLevels={gridLevels} loading={loadingTrades} />
        )}

        {/* Trades tab */}
        {activeTab === 'trades' && (
          <div className="max-w-7xl mx-auto px-4 md:px-8 py-6">
            {loadingTrades ? (
              <div className="flex justify-center py-12">
                <svg className="animate-spin h-5 w-5 text-on-surface-tertiary" viewBox="0 0 24 24" fill="none">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
              </div>
            ) : trades.length > 0 ? (
              <div className="rounded-xl border border-line-default bg-surface-1 overflow-hidden shadow-sm">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
                      <th className="text-right px-4 py-2.5 font-medium">层级</th>
                      <th className="text-left px-3 py-2.5 font-medium">开仓</th>
                      <th className="text-right px-3 py-2.5 font-medium">开仓价</th>
                      <th className="text-right px-3 py-2.5 font-medium">开仓量</th>
                      <th className="text-left px-3 py-2.5 font-medium">平仓</th>
                      <th className="text-right px-3 py-2.5 font-medium">平仓价</th>
                      <th className="text-right px-3 py-2.5 font-medium">平仓量</th>
                      <th className="text-right px-3 py-2.5 font-medium">盈亏</th>
                      <th className="text-left px-3 py-2.5 font-medium">状态</th>
                    </tr>
                  </thead>
                  <tbody>
                    {trades.map((t) => (
                      <tr key={t.id} className="border-b border-line-subtle">
                        <td className="px-4 py-2 text-on-surface-secondary font-mono text-right">{t.grid_level}</td>
                        <td className="px-3 py-2">
                          <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.open_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>
                            {t.open_side === 'buy' ? '买入' : '卖出'}
                          </span>
                        </td>
                        <td className="px-3 py-2 text-on-surface font-mono text-right">{t.open_price.toFixed(2)}</td>
                        <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">{t.open_quantity.toFixed(4)}</td>
                        <td className="px-3 py-2">
                          {t.close_side ? (
                            <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.close_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>
                              {t.close_side === 'buy' ? '买入' : '卖出'}
                            </span>
                          ) : (
                            <span className="text-on-surface-faint">-</span>
                          )}
                        </td>
                        <td className="px-3 py-2 text-on-surface font-mono text-right">{t.close_price?.toFixed(2) || '-'}</td>
                        <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">{t.close_quantity?.toFixed(4) || '-'}</td>
                        <td className="px-3 py-2 text-right">{formatPnlShort(t.pnl)}</td>
                        <td className="px-3 py-2">
                          <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                            t.status === 'closed' ? 'bg-emerald-500/10 text-emerald-400'
                            : t.status === 'open' ? 'bg-blue-500/10 text-blue-400'
                            : 'bg-surface-2 text-on-surface-tertiary'
                          }`}>
                            {t.status === 'closed' ? '已平' : t.status === 'open' ? '持仓' : t.status}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="text-center py-12 text-on-surface-tertiary text-sm">暂无交易记录</div>
            )}
          </div>
        )}

        {/* Analysis tab */}
        {activeTab === 'analysis' && (
          <AnalysisList logs={analysisLogs} loading={loadingAnalysis} botType="grid" botId={params.id!} />
        )}
      </div>
    </div>
  );
}

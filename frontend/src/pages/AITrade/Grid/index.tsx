import { useState, useEffect, useRef, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { getGridAnalysisLogs, startGridBot, stopGridBot, deleteGridBot } from '../../../service/bot'
import { fetchKlines, fetchOrderBook } from '../../../service/market'
import type {
  AnalysisLog,
  GridBot,
  GridTrade,
  KlineCandle,
  OrderBookData,
} from '../../../service/types'
import {
  useKlineWs,
  useOrderBookWs,
  type KlineWsEvent,
  type OrderBookWsEvent,
} from '../../../service/ws'
import ChartPanel from '../components/ChartPanel'
import type { KlineChartHandle } from '../../../components/Chart/KlineChart'
import OrderBookPanel from '../components/OrderBookPanel'
import AnalysisList from '../components/AnalysisList'
import SystemInfo from '../components/SystemInfo'
import GridStats from './GridStats'
import MobileOrderBook from '../components/MobileOrderBook'
import MarketIndicators from '../components/MarketIndicators'
import GridLevelsTab from './GridLevelsTab'
import { formatPnlShort } from '../components/shared'
import { useBot } from '../context/BotContext'
import { useHeader, type ItemConfig } from '../components/Header/context'

// ── Page ──────────────────────────────────────────────────

export default function GridDetailPage() {
  const navigate = useNavigate()
  const params = useParams()
  const { activeTab, updateActiveTab, updateTabs, updateActions } = useHeader()
  const { bot, trades, gridLevels, loading } = useBot()

  const [analysisLogs, setAnalysisLogs] = useState<AnalysisLog[]>([])
  const [loadingAnalysis, setLoadingAnalysis] = useState(false)

  // Market data
  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [klineTimeframe, setKlineTimeframe] = useState('15m')
  const [orderBook, setOrderBook] = useState<OrderBookData>({ bids: [], asks: [] })

  const chartRef = useRef<KlineChartHandle>(null)

  const loadAnalysis = useCallback(async () => {
    if (!bot?.id) return

    setLoadingAnalysis(true)
    try {
      const res = await getGridAnalysisLogs(bot?.id)
      setAnalysisLogs(res.data?.items || [])
    } catch (e) {
      console.error(e)
    } finally {
      setLoadingAnalysis(false)
    }
  }, [bot?.id])

  const loadKlines = useCallback(
    async (exchange: string, symbol: string, market_type: string, tf: string) => {
      try {
        const res = await fetchKlines({ exchange, symbol, market_type, timeframe: tf })
        if (res.data) setKlineData(res.data)
      } catch (e) {
        console.error('Failed to load kline:', e)
      }
    },
    [],
  )

  const loadKlineStable = useCallback(() => {
    if (!bot?.exchange || !bot?.symbol || !bot?.market_type || !klineTimeframe) return
    loadKlines(bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe)
  }, [bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe, loadKlines])

  const loadOrderBook = useCallback(
    async (exchange: string, symbol: string, market_type: string) => {
      try {
        // 确保后端订阅了该 symbol 的订单簿流（后端重启后订阅会丢失）
        const res = await fetchOrderBook({ exchange, symbol, market_type })
        if (res.data) setOrderBook(res.data)
      } catch (e) {
        console.error('Failed to load orderbook:', e)
      }
    },
    [],
  )

  useEffect(() => {
    if (!bot?.id) return

    updateTabs([
      {
        key: 'market',
        label: '行情',
        onClick: () => {
          navigate(`/trade/grid/${bot?.id}`, { replace: true })
        },
      },
      {
        key: 'trades',
        label: '交易记录',
        onClick: () => {
          navigate(`/trade/grid/${bot?.id}/trades`, { replace: true })
        },
      },
      {
        key: 'analysis',
        label: 'AI 决策日志',
        onClick: () => {
          loadAnalysis()
          navigate(`/trade/grid/${bot?.id}/analysis`, { replace: true })
        },
      },
      {
        key: 'system',
        label: '系统',
        onClick: () => {
          navigate(`/trade/grid/${bot?.id}/system`, { replace: true })
        },
      },
    ])

    updateActiveTab('market')
  }, [bot?.id])

  useEffect(() => {
    if (!bot?.id || !bot?.status) return
    const actions: ItemConfig[] = []
    actions.push({
      key: 'delete',
      label: '删除',
      onClick: async () => {
        if (!confirm('确定删除此机器人？')) return
        await stopGridBot(bot?.id).catch(() => {})
        await deleteGridBot(bot?.id)
        navigate('/setup/bot-type', { replace: true })
      },
    })
    if (bot?.status === 'running') {
      actions.push({
        key: 'stop',
        label: '暂停',
        onClick: async () => {
          await stopGridBot(bot?.id)
        },
      })
    }
    if (bot?.status === 'stopped') {
      actions.push({
        key: 'start',
        label: '运行',
        onClick: async () => {
          await startGridBot(bot?.id)
        },
      })
    }
    updateActions(actions)
  }, [bot?.id, bot?.status])

  useEffect(() => {
    if (!bot?.exchange || !bot?.symbol || !bot?.market_type || !klineTimeframe) return
    loadKlines(bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe)
  }, [bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe])

  useEffect(() => {
    if (!bot?.exchange || !bot?.symbol || !bot?.market_type) return
    loadOrderBook(bot?.exchange, bot?.symbol, bot?.market_type)
  }, [bot?.exchange, bot?.symbol, bot?.market_type])

  // Real-time orderbook via WebSocket (replaces 2s polling)
  useOrderBookWs((event: OrderBookWsEvent) => {
    if (!bot) return

    if (event.symbol !== bot?.symbol || event.exchange !== bot?.exchange) return

    setOrderBook(event.orderBook)
  })

  useKlineWs(
    (event: KlineWsEvent) => {
      if (!bot) return
      if (event.symbol !== bot?.symbol || event.exchange !== bot?.exchange) return

      const c = event.candle
      if (!c) return
      // Update chart directly via series.update() — no re-render
      chartRef.current?.update(c)
    },
    loadKlineStable,
    klineTimeframe,
  )

  if (!bot) {
    return (
      <div className="flex-1 flex items-center justify-center relative z-10 h-screen bg-base">
        <svg
          className="animate-spin h-5 w-5 text-on-surface-tertiary"
          viewBox="0 0 24 24"
          fill="none"
        >
          <circle
            className="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            strokeWidth="4"
          />
          <path
            className="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          />
        </svg>
      </div>
    )
  }

  return (
    <>
      {/* Market tab */}
      {activeTab === 'market' && (
        <div className="h-full flex flex-col lg:flex-row">
          <div className="flex flex-col h-full lg:flex-1 lg:min-h-0">
            <GridStats bot={bot as GridBot} />
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
              {(trades as GridTrade[]).length > 0 && (
                <div className="px-4 py-2">
                  <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">
                    最近成交
                  </div>
                  {(trades as GridTrade[]).slice(0, 5).map((t, i) => (
                    <div key={i} className="flex items-center justify-between py-1 text-xs">
                      <span className={t.open_side === 'buy' ? 'text-emerald-400' : 'text-red-400'}>
                        {t.open_side === 'buy' ? '买' : '卖'} {t.open_quantity?.toFixed(4) || '-'} @{' '}
                        {t.open_price?.toFixed(2) || '-'}
                      </span>
                      <span className="text-on-surface-tertiary text-[10px]">
                        {new Date(t.opened_at).toLocaleTimeString('zh-CN', {
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
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
      {activeTab === 'levels' && <GridLevelsTab gridLevels={gridLevels} loading={loading} />}

      {/* Trades tab */}
      {activeTab === 'trades' && (
        <div className="max-w-7xl mx-auto px-4 md:px-8 py-6">
          {loading ? (
            <div className="flex justify-center py-12">
              <svg
                className="animate-spin h-5 w-5 text-on-surface-tertiary"
                viewBox="0 0 24 24"
                fill="none"
              >
                <circle
                  className="opacity-25"
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  strokeWidth="4"
                />
                <path
                  className="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                />
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
                  {(trades as GridTrade[]).map((t) => (
                    <tr key={t.id} className="border-b border-line-subtle">
                      <td className="px-4 py-2 text-on-surface-secondary font-mono text-right">
                        {t.grid_level}
                      </td>
                      <td className="px-3 py-2">
                        <span
                          className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.open_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                        >
                          {t.open_side === 'buy' ? '买入' : '卖出'}
                        </span>
                      </td>
                      <td className="px-3 py-2 text-on-surface font-mono text-right">
                        {t.open_price.toFixed(2)}
                      </td>
                      <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                        {t.open_quantity.toFixed(4)}
                      </td>
                      <td className="px-3 py-2">
                        {t.close_side ? (
                          <span
                            className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.close_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                          >
                            {t.close_side === 'buy' ? '买入' : '卖出'}
                          </span>
                        ) : (
                          <span className="text-on-surface-faint">-</span>
                        )}
                      </td>
                      <td className="px-3 py-2 text-on-surface font-mono text-right">
                        {t.close_price?.toFixed(2) || '-'}
                      </td>
                      <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                        {t.close_quantity?.toFixed(4) || '-'}
                      </td>
                      <td className="px-3 py-2 text-right">{formatPnlShort(t.pnl)}</td>
                      <td className="px-3 py-2">
                        <span
                          className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                            t.status === 'closed'
                              ? 'bg-emerald-500/10 text-emerald-400'
                              : t.status === 'open'
                                ? 'bg-blue-500/10 text-blue-400'
                                : 'bg-surface-2 text-on-surface-tertiary'
                          }`}
                        >
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
        <AnalysisList
          logs={analysisLogs}
          loading={loadingAnalysis}
          botType="grid"
          botId={params.botId!}
        />
      )}

      {/* System tab */}
      {activeTab === 'system' && <SystemInfo />}
    </>
  )
}

import { useState, useEffect, useRef, useCallback, memo } from 'react'
import { useNavigate } from 'react-router-dom'
import { getAutoAnalysisLogs, startAutoBot, stopAutoBot, deleteAutoBot } from '../../../service/bot'
import { fetchKlines, fetchOrderBook } from '../../../service/market'
import type {
  AnalysisLog,
  AutoBot,
  AutoTrade,
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
import PositionStats from './PositionStats'
import MobileOrderBook from '../components/MobileOrderBook'
import MarketIndicators from '../components/MarketIndicators'
import { formatPnl } from '../components/shared'
import { useBot } from '../context/BotContext'
import { useHeader, type ItemConfig } from '../components/Header/context'

// ── Helpers ───────────────────────────────────────────────
const tradeTypeLabel = (t: string) => {
  const map: Record<string, string> = {
    open_long: '开多',
    open_short: '开空',
    close_long: '平多',
    close_short: '平空',
    stop_loss: '止损',
    take_profit: '止盈',
  }
  return map[t] || t
}

const tradeTypeColor = (t: string) => {
  if (t.startsWith('open_long') || t === 'close_short') return 'text-emerald-400'
  if (t.startsWith('open_short') || t === 'close_long') return 'text-red-400'
  if (t === 'stop_loss') return 'text-red-400'
  if (t === 'take_profit') return 'text-emerald-400'
  return 'text-on-surface-tertiary'
}

// ── Page ──────────────────────────────────────────────────
const AutoDetailPage = () => {
  console.log('进入页面')
  const navigate = useNavigate()
  const { activeTab, updateActiveTab, updateTabs, updateActions } = useHeader()
  const { bot, trades, loading } = useBot()
  const [logs, setLogs] = useState<AnalysisLog[]>([])

  // Market data
  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [klineTimeframe, setKlineTimeframe] = useState('15m')
  const [orderBook, setOrderBook] = useState<OrderBookData>({ bids: [], asks: [] })

  const chartRef = useRef<KlineChartHandle>(null)

  const loadLogs = useCallback(async (botId: string) => {
    try {
      const res = await getAutoAnalysisLogs(botId)
      if (res.data?.logs) setLogs(res.data.logs)
    } catch (e) {
      console.error('Failed to load analysis logs:', e)
    }
  }, [])

  const loadKlines = useCallback(
    async (exchange: string, symbol: string, market_type: string, tf: string) => {
      try {
        const res = await fetchKlines({
          exchange,
          symbol,
          market_type,
          timeframe: tf,
        })
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
        const res = await fetchOrderBook({
          exchange,
          symbol,
          market_type,
        })
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
          updateActiveTab('market')
          navigate(`/trade/auto/${bot?.id}`, { replace: true })
        },
      },
      {
        key: 'trades',
        label: '交易记录',
        onClick: () => {
          updateActiveTab('trades')
          navigate(`/trade/auto/${bot?.id}/trades`, { replace: true })
        },
      },
      {
        key: 'analysis',
        label: 'AI 决策日志',
        onClick: () => {
          updateActiveTab('analysis')
          navigate(`/trade/auto/${bot?.id}/analysis`, { replace: true })
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
      className:
        'bg-red-500/10 border-red-500/20 text-red-400 hover:bg-red-500/20 transition-colors',
      onClick: async () => {
        if (!confirm('确定删除此机器人？将平仓所有持仓。')) return
        await deleteAutoBot(bot?.id)
        navigate('/setup/bot-type', { replace: true })
      },
    })

    if (bot?.status === 'running') {
      actions.push({
        key: 'stop',
        label: '暂停',
        className:
          'bg-surface-1 border-line-default text-on-surface-tertiary hover:text-red-400 hover:border-red-500/20 transition-colors',
        onClick: async () => {
          await stopAutoBot(bot?.id)
        },
      })
    }

    if (bot?.status === 'stopped') {
      actions.push({
        key: 'start',
        label: '运行',
        className:
          'bg-emerald-500/10 border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20 transition-colors',
        onClick: async () => {
          await startAutoBot(bot?.id)
        },
      })
    }

    updateActions(actions)
    loadKlineStable()
  }, [bot?.id])

  useEffect(() => {
    if (!bot?.id) return
    loadLogs(bot?.id)
  }, [bot?.id, loadLogs])

  useEffect(() => {
    if (!bot?.symbol || !bot?.exchange || !bot?.market_type) return
    loadOrderBook(bot?.exchange, bot?.symbol, bot?.market_type)
  }, [bot?.exchange, bot?.symbol, bot?.market_type, loadOrderBook])

  useEffect(() => {
    if (!bot?.exchange || !bot?.symbol || !bot?.market_type || !klineTimeframe) return
    loadKlines(bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe)
  }, [bot?.exchange, bot?.symbol, bot?.market_type, klineTimeframe, loadKlines])

  useOrderBookWs((event: OrderBookWsEvent) => {
    if (!bot) return
    if (event.symbol !== bot?.symbol || event.exchange !== bot?.exchange) return

    setOrderBook(event.orderBook)
  })

  useKlineWs((event: KlineWsEvent) => {
    if (!bot) return
    if (
      event.symbol !== bot?.symbol ||
      event.exchange !== bot?.exchange ||
      klineTimeframe !== event.timeframe
    )
      return
    const c = event.candle
    if (!c) return
    // Update chart directly via series.update() — no re-render
    chartRef.current?.update(c)
  }, loadKlineStable)

  if (loading || !bot) {
    return (
      <div className="h-screen bg-base flex flex-col items-center justify-center relative gap-4">
        {!loading && !bot ? (
          <>
            <div className="text-on-surface-tertiary text-sm">{'机器人不存在或加载失败'}</div>
            <button
              onClick={() => navigate('/setup/bot-type', { replace: true })}
              className="px-4 py-2 text-xs font-medium border border-line-default rounded-lg text-on-surface-tertiary hover:bg-surface-2 transition-colors"
            >
              创建新机器人
            </button>
          </>
        ) : (
          <svg
            className="animate-spin h-6 w-6 text-on-surface-tertiary"
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
        )}
      </div>
    )
  }

  return (
    <>
      {activeTab === 'market' && (
        <div className="h-full flex flex-col lg:flex-row">
          <div className="flex flex-col h-full lg:flex-1 lg:min-h-0">
            <PositionStats bot={bot as AutoBot} />
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
              {(trades as AutoTrade[]).length > 0 && (
                <div className="px-4 py-2">
                  <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">
                    最近成交
                  </div>
                  {(trades as AutoTrade[]).slice(0, 8).map((t) => (
                    <div key={t.id} className="flex items-center justify-between py-1 text-xs">
                      <span className={t.side === 'buy' ? 'text-emerald-400' : 'text-red-400'}>
                        {t.side === 'buy' ? '买' : '卖'} {t.quantity.toFixed(4)} @{' '}
                        {t.price.toFixed(2)}
                      </span>
                      <span className="text-on-surface-tertiary text-[10px]">
                        {new Date(t.created_at).toLocaleTimeString('zh-CN', {
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

      {/* Trades tab */}
      {activeTab === 'trades' && (
        <div className="max-w-5xl mx-auto px-4 md:px-8 py-6">
          {trades.length > 0 ? (
            <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
              <div className="divide-y divide-line-subtle">
                {(trades as AutoTrade[]).slice(0, 50).map((t) => (
                  <div
                    key={t.id}
                    className="flex items-center justify-between px-5 py-3 hover:bg-surface-2/50"
                  >
                    <div className="flex items-center gap-3">
                      <span
                        className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${tradeTypeColor(t.trade_type)} bg-surface-2`}
                      >
                        {tradeTypeLabel(t.trade_type)}
                      </span>
                      <div>
                        <div className="text-xs text-on-surface font-mono">
                          {t.side === 'buy' ? '买入' : '卖出'} {t.quantity.toFixed(6)} @{' '}
                          {t.price.toFixed(2)}
                        </div>
                        <div className="text-[10px] text-on-surface-tertiary mt-0.5">
                          {new Date(t.created_at).toLocaleString('zh-CN')}
                        </div>
                      </div>
                    </div>
                    {t.pnl !== 0 && (
                      <div className="text-right">
                        {formatPnl(t.pnl)}
                        {t.pnl_pct !== 0 && (
                          <div className="text-[10px] text-on-surface-tertiary">
                            {t.pnl_pct.toFixed(2)}%
                          </div>
                        )}
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
        <AnalysisList logs={logs} loading={false} botType="auto" botId={bot?.id} />
      )}
    </>
  )
}

export default memo(AutoDetailPage)

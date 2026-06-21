import { useState, useEffect, useRef, useCallback, memo, useMemo } from 'react'
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
import type { KlineChartHandle } from '../../../components/Chart/KlineChart'
import AnalysisList from '../components/AnalysisList'
import SystemInfo from '../components/SystemInfo'
import BotSidebar from '../components/BotSidebar'
import AIDecisionCard from '../components/AIDecisionCard'
import TradeStats from '../components/TradeStats'
import CollapsibleMarketPanel from '../components/CollapsibleMarketPanel'
import PositionStats from './PositionStats'
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

/**
 * 把交易记录转换为 K线图 markers。
 * 买入（side=buy）→ 绿色向上箭头，位于 K线下方
 * 卖出（side=sell）→ 红色向下箭头，位于 K线上方
 */
function tradesToMarkers(trades: AutoTrade[]) {
  return trades
    .map((t) => {
      const time = Math.floor(new Date(t.created_at).getTime() / 1000)
      const isBuy = t.side === 'buy'
      return {
        time,
        position: isBuy ? ('belowBar' as const) : ('aboveBar' as const),
        color: isBuy ? '#10b981' : '#ef4444',
        shape: isBuy ? ('arrowUp' as const) : ('arrowDown' as const),
        text: `${isBuy ? '买' : '卖'} ${t.price.toFixed(2)}`,
      }
    })
    .sort((a, b) => a.time - b.time)
}

// ── Page ──────────────────────────────────────────────────
const AutoDetailPage = () => {
  const navigate = useNavigate()
  const { activeTab, updateActiveTab, updateTabs, updateActions } = useHeader()
  const { bot, trades, loading } = useBot()
  const [logs, setLogs] = useState<AnalysisLog[]>([])

  // Market data
  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [klineTimeframe, setKlineTimeframe] = useState('15m')
  const [orderBook, setOrderBook] = useState<OrderBookData>({ bids: [], asks: [] })
  const [latestPrice, setLatestPrice] = useState(0)

  const chartRef = useRef<KlineChartHandle>(null)

  // 把交易记录转换为 K线图 markers
  const markers = useMemo(() => tradesToMarkers(trades as AutoTrade[]), [trades])

  // 最近一次 AI 决策
  const latestDecision = logs[0] || null

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
        key: 'bot',
        label: '机器人',
        onClick: () => {
          updateActiveTab('bot')
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
      {
        key: 'system',
        label: '系统',
        onClick: () => {
          updateActiveTab('system')
          navigate(`/trade/auto/${bot?.id}/system`, { replace: true })
        },
      },
    ])

    updateActiveTab('bot')
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
  }, [bot?.id, bot?.status])

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

  useKlineWs(
    (event: KlineWsEvent) => {
      if (!bot) return
      if (event.symbol !== bot?.symbol || event.exchange !== bot?.exchange) return
      const c = event.candle
      if (!c) return
      // 更新最新价
      setLatestPrice(c.close)
      // Update chart directly via series.update() — no re-render
      chartRef.current?.update(c)
    },
    loadKlineStable,
    klineTimeframe,
  )

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

  const autoBot = bot as AutoBot

  return (
    <>
      {activeTab === 'bot' && (
        <div className="h-full flex flex-col lg:flex-row">
          {/* 主区域：状态栏 + AI决策 + 交易统计 + 底部行情折叠 */}
          <div className="flex flex-col h-full lg:flex-1 lg:min-h-0 overflow-y-auto">
            <PositionStats bot={autoBot} latestPrice={latestPrice} trades={trades as AutoTrade[]} />

            {/* AI 决策卡片 */}
            <AIDecisionCard log={latestDecision} botId={bot?.id} botType="auto" />

            {/* 历史交易统计 */}
            <TradeStats
              trades={trades as AutoTrade[]}
              totalTrades={autoBot.total_trades}
              winTrades={autoBot.win_trades}
              lossTrades={autoBot.loss_trades}
            />

            {/* 底部行情折叠面板（K线图 + 订单簿） */}
            <div className="mt-auto">
              <CollapsibleMarketPanel
                klineData={klineData}
                klineTimeframe={klineTimeframe}
                onTimeframeChange={setKlineTimeframe}
                chartRef={chartRef}
                markers={markers}
                orderBook={orderBook}
                latestPrice={latestPrice}
              />
            </div>
          </div>

          {/* 右侧侧边栏：最近决策 + 最近成交 */}
          <div className="hidden lg:flex w-72 xl:w-80 border-l border-line-subtle flex-col">
            <BotSidebar logs={logs} trades={trades as AutoTrade[]} botId={bot?.id} botType="auto" />
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
                          {t.fee > 0 && (
                            <span className="text-amber-400 ml-2">手续费 {t.fee.toFixed(4)}</span>
                          )}
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

      {/* System tab */}
      {activeTab === 'system' && <SystemInfo />}
    </>
  )
}

export default memo(AutoDetailPage)

import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  fetchKlines,
  fetchOrderBook,
  getAutoAnalysisLogs,
  type AnalysisLog,
  type AutoBot,
  type AutoTrade,
  type KlineCandle,
  type OrderBookData,
} from '@/service'
import { useKlineWs, useOrderBookWs, type KlineWsEvent, type OrderBookWsEvent } from '@/service/ws'
import type { KlineChartHandle } from '@/components/Chart/KlineChart'

import AIDecisionCard from '../../components/AIDecisionCard'
import AITradeStatsCard from '../../components/AITradeStatsCard'
import CollapsibleMarketPanel from '../../components/CollapsibleMarketPanel'
import { useBot } from '../../context/BotContext'
import PositionStats from './PositionStats'
import AIRecentDecisionsCard from '../../components/AIRecentDecisionsCard'
import AIRecentTradesCard from '../../components/AIRecentTradesCard'

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

const Bot = () => {
  const { bot, trades } = useBot()

  const [klineTimeframe, setKlineTimeframe] = useState('15m')
  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [latestPrice, setLatestPrice] = useState(0)
  const [orderBook, setOrderBook] = useState<OrderBookData>({ bids: [], asks: [] })
  const [logs, setLogs] = useState<AnalysisLog[]>([])

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

  const autoBot = useMemo(() => bot as AutoBot, [bot])
  const autoTrades = useMemo(() => trades as AutoTrade[], [trades])

  const markers = useMemo(() => tradesToMarkers(autoTrades), [autoTrades])
  const latestDecision = useMemo(() => logs[0] || null, [logs])

  return (
    <div className="h-full flex flex-col lg:flex-row">
      {/* 主区域：状态栏 + AI决策 + 交易统计 + 底部行情折叠 */}
      <div className="flex flex-col h-full lg:flex-1 lg:min-h-0 overflow-y-auto">
        {/* 仓位状态 */}
        <PositionStats bot={autoBot} latestPrice={latestPrice} trades={autoTrades} />

        {/* AI 决策卡片 */}
        <AIDecisionCard log={latestDecision} botId={autoBot?.id} botType="auto" />

        {/* 历史交易统计 */}
        <AITradeStatsCard
          trades={autoTrades}
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
        <div className="flex flex-col h-full divide-y divide-line-subtle">
          <div className="flex-1 min-h-0">
            <AIRecentDecisionsCard logs={logs} botId={autoBot?.id} botType={'auto'} />
          </div>
          <div className="flex-1 min-h-0">
            <AIRecentTradesCard trades={autoTrades} />
          </div>
        </div>
      </div>
    </div>
  )
}

export default memo(Bot)

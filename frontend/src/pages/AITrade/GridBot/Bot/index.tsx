import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useBot } from '../../context/BotContext'
import GridStats from './GridStats'
import {
  fetchKlines,
  fetchOrderBook,
  useKlineWs,
  type GridBot,
  type GridTrade,
  type KlineCandle,
  type KlineWsEvent,
  type OrderBookData,
} from '@/service'
import ChartPanel from '../../components/ChartPanel'
import MarketIndicators from '../../components/MarketIndicators'
import MobileOrderBook from '../../components/MobileOrderBook'
import OrderBookPanel from '../../components/OrderBookPanel'
import type { KlineChartHandle } from '@/components/Chart/KlineChart'
import { useOrderBookWs, type OrderBookWsEvent } from '@/service/ws'

const Bot = () => {
  const { bot, trades } = useBot()

  const [klineData, setKlineData] = useState<KlineCandle[]>([])
  const [klineTimeframe, setKlineTimeframe] = useState('15m')
  const [orderBook, setOrderBook] = useState<OrderBookData>({ bids: [], asks: [] })

  const chartRef = useRef<KlineChartHandle>(null)

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

  const gridBot = useMemo(() => bot as GridBot, [bot])
  const gridTrades = useMemo(() => trades as GridTrade[], [trades])

  return (
    <div className="h-full flex flex-col lg:flex-row">
      <div className="flex flex-col h-full lg:flex-1 lg:min-h-0">
        <GridStats bot={gridBot} />
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
          {gridTrades.length > 0 && (
            <div className="px-4 py-2">
              <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-1.5">
                最近成交
              </div>
              {gridTrades.slice(0, 5).map((t, i) => (
                <div key={i} className="flex items-center justify-between py-1 text-xs">
                  <span
                    className={`font-mono ${t.open_side === 'buy' ? 'text-emerald-400' : 'text-red-400'}`}
                  >
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
  )
}

export default memo(Bot)

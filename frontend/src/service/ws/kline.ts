import { useEffect } from 'react'
import { createWsInstance, useWsHook } from '../../lib/ws'

export interface KlineWsEventRaw {
  exchange: string
  symbol: string
  timeframe: string
  candle: {
    open_time: number
    close_time: number
    open: number
    high: number
    low: number
    close: number
    volume: number
    quote_volume: number
    trades: number
    closed: boolean
  }
  event_type: 'Update' | 'Closed' | 'Backfilled'
}

// ── Kline event type (aligned with backend KlineEngine) ───

export interface KlineWsEvent {
  exchange: string
  symbol: string
  timeframe: string
  candle: {
    close: number
    high: number
    low: number
    open: number
    volume: number
    time: number
  }
  event_type: 'Update' | 'Closed' | 'Backfilled'
}

// ── Kline WebSocket ────────────────────────────────────────

const klineInst = createWsInstance<KlineWsEvent>()

function getKlineWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}/ws/kline`
}

const parseKlineWs = (raw: string): KlineWsEvent | null => {
  try {
    const json = JSON.parse(raw) as KlineWsEventRaw
    return {
      ...json,
      candle: {
        close: json?.candle?.close,
        high: json?.candle?.high,
        low: json?.candle?.low,
        open: json?.candle?.open,
        time: json?.candle?.open_time / 1000,
        volume: json?.candle?.volume,
      },
    }
  } catch {
    return null
  }
}

/**
 * 订阅 Kline WebSocket。
 * 支持传入 timeframe，后端将只推送该周期的数据，节省约 83% 带宽。
 * 切换 timeframe 时会自动发送新的订阅消息，无需重连。
 */
export function useKlineWs(
  onEvent: (event: KlineWsEvent) => void,
  onReconnect?: () => void,
  timeframe?: string,
): { connected: boolean } {
  const { connected } = useWsHook(klineInst, getKlineWsUrl, parseKlineWs, onEvent, onReconnect)

  // 当 timeframe 变化时，发送订阅消息给后端
  useEffect(() => {
    if (!connected) return
    const ws = klineInst.ws
    if (!ws || ws.readyState !== WebSocket.OPEN) return

    if (timeframe) {
      ws.send(JSON.stringify({ action: 'subscribe', timeframe }))
    } else {
      ws.send(JSON.stringify({ action: 'unsubscribe' }))
    }
  }, [timeframe, connected])

  return { connected }
}

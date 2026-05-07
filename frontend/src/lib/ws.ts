import { createSignal, onCleanup } from 'solid-js'

export interface GridBotEvent {
  type: 'grid_bot'
  data: {
    bot_id: string
    name: string
    status: 'running' | 'stopped' | 'error'
  }
}

export interface TradeEvent {
  type: 'trade'
  data: {
    bot_id: string
    symbol: string
    side: string
    price: number
    quantity: number
    pnl: number
  }
}

export interface OrderEvent {
  type: 'order'
  data: {
    order_id: string
    bot_id: string
    symbol: string
    status: string
    error?: string
  }
}

export interface PositionEvent {
  type: 'position'
  data: {
    bot_id: string
    symbol: string
    side: string
    size: number
    entry_price: number
    action: 'opened' | 'closed'
  }
}

export interface RiskEvent {
  type: 'risk'
  data: {
    bot_id: string
    symbol: string
    reason: 'stop_loss' | 'take_profit' | 'trailing_stop'
    price: number
  }
}

export interface NotificationEvent {
  type: 'notification'
  data: {
    level: 'info' | 'warning' | 'error'
    message: string
  }
}

export interface BotStatusEvent {
  type: 'bot_status'
  data: {
    bot_id: string
    name: string
    status: 'running' | 'stopped' | 'error'
  }
}

export type WsEvent =
  | BotStatusEvent
  | TradeEvent
  | OrderEvent
  | PositionEvent
  | RiskEvent
  | NotificationEvent

export interface KlineWsEvent {
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

const TOKEN_KEY = 'qd_token'
const BASE_RECONNECT_MS = 1000
const MAX_RECONNECT_MS = 30000

let globalWs: WebSocket | null = null
let globalListeners: Array<(event: WsEvent) => void> = []
let reconnectTimer: ReturnType<typeof setTimeout> | null = null
let reconnectAttempts = 0
const [connected, setConnected] = createSignal(false)

function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY)
}

function getWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const token = getToken()
  const base = `${protocol}//${window.location.host}/ws`
  if (token) {
    return `${base}?token=${encodeURIComponent(token)}`
  }
  return base
}

function connect() {
  if (globalWs && globalWs.readyState === WebSocket.OPEN) return

  const token = getToken()
  if (!token) return

  try {
    globalWs = new WebSocket(getWsUrl())

    globalWs.onopen = () => {
      setConnected(true)
      reconnectAttempts = 0
    }

    globalWs.onmessage = (e) => {
      try {
        const event: WsEvent = JSON.parse(e.data)
        globalListeners.forEach(listener => listener(event))
      } catch (err) {
        console.error('[WS] Failed to parse message:', err)
      }
    }

    globalWs.onclose = () => {
      setConnected(false)
      const delay = Math.min(BASE_RECONNECT_MS * Math.pow(2, reconnectAttempts), MAX_RECONNECT_MS)
      reconnectAttempts++
      reconnectTimer = setTimeout(connect, delay)
    }

    globalWs.onerror = () => {}
  } catch (err) {
    console.error('[WS] Failed to connect:', err)
    const delay = Math.min(BASE_RECONNECT_MS * Math.pow(2, reconnectAttempts), MAX_RECONNECT_MS)
    reconnectAttempts++
    reconnectTimer = setTimeout(connect, delay)
  }
}

export function useWs(onEvent: (event: WsEvent) => void): { connected: () => boolean } {
  globalListeners.push(onEvent)

  if (!globalWs || globalWs.readyState === WebSocket.CLOSED) {
    reconnectAttempts = 0
    connect()
  }

  onCleanup(() => {
    globalListeners = globalListeners.filter(l => l !== onEvent)
    if (globalListeners.length === 0 && globalWs) {
      if (reconnectTimer) clearTimeout(reconnectTimer)
      globalWs.close()
      globalWs = null
    }
  })

  return { connected }
}

let klineWs: WebSocket | null = null
let klineListeners: Array<(event: KlineWsEvent) => void> = []
let klineReconnectTimer: ReturnType<typeof setTimeout> | null = null
let klineReconnectAttempts = 0
const [klineConnected, setKlineConnected] = createSignal(false)
let klineReconnectCallbacks: Array<() => void> = []

function getKlineWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}/ws/kline`
}

function connectKlineWs() {
  if (klineWs && klineWs.readyState === WebSocket.OPEN) return

  try {
    klineWs = new WebSocket(getKlineWsUrl())

    klineWs.onopen = () => {
      setKlineConnected(true)
      klineReconnectAttempts = 0
      klineReconnectCallbacks.forEach(cb => cb())
    }

    klineWs.onmessage = (e) => {
      try {
        const event: KlineWsEvent = JSON.parse(e.data)
        klineListeners.forEach(listener => listener(event))
      } catch (err) {
        console.error('[KlineWS] Failed to parse message:', err)
      }
    }

    klineWs.onclose = () => {
      setKlineConnected(false)
      const delay = Math.min(BASE_RECONNECT_MS * Math.pow(2, klineReconnectAttempts), MAX_RECONNECT_MS)
      klineReconnectAttempts++
      klineReconnectTimer = setTimeout(connectKlineWs, delay)
    }

    klineWs.onerror = () => {}
  } catch (err) {
    console.error('[KlineWS] Failed to connect:', err)
    const delay = Math.min(BASE_RECONNECT_MS * Math.pow(2, klineReconnectAttempts), MAX_RECONNECT_MS)
    klineReconnectAttempts++
    klineReconnectTimer = setTimeout(connectKlineWs, delay)
  }
}

export function useKlineWs(
  onEvent: (event: KlineWsEvent) => void,
  onReconnect?: () => void,
): { connected: () => boolean } {
  klineListeners.push(onEvent)

  if (onReconnect) {
    klineReconnectCallbacks.push(onReconnect)
  }

  if (!klineWs || klineWs.readyState === WebSocket.CLOSED) {
    klineReconnectAttempts = 0
    connectKlineWs()
  }

  onCleanup(() => {
    klineListeners = klineListeners.filter(l => l !== onEvent)
    if (onReconnect) {
      klineReconnectCallbacks = klineReconnectCallbacks.filter(cb => cb !== onReconnect)
    }
    if (klineListeners.length === 0 && klineWs) {
      if (klineReconnectTimer) clearTimeout(klineReconnectTimer)
      klineWs.close()
      klineWs = null
    }
  })

  return { connected: klineConnected }
}

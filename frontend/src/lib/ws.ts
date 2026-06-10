import { createSignal, onCleanup } from 'solid-js'

// ── Event types ────────────────────────────────────────────

export interface BotStatusEvent {
  type: 'bot_status'
  data: { bot_id: string; name: string; status: 'running' | 'stopped' | 'error' }
}

export interface TradeEvent {
  type: 'trade'
  data: { bot_id: string; symbol: string; side: string; price: number; quantity: number; pnl: number }
}

export interface OrderEvent {
  type: 'order'
  data: { order_id: string; bot_id: string; symbol: string; status: string; error?: string }
}

export interface PositionEvent {
  type: 'position'
  data: { bot_id: string; symbol: string; side: string; size: number; entry_price: number; action: 'opened' | 'closed' }
}

export interface RiskEvent {
  type: 'risk'
  data: { bot_id: string; symbol: string; reason: 'stop_loss' | 'take_profit' | 'trailing_stop'; price: number }
}

export interface NotificationEvent {
  type: 'notification'
  data: { level: 'info' | 'warning' | 'error'; message: string }
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

// ── WebSocket factory ──────────────────────────────────────

interface WsInstance<T> {
  ws: WebSocket | null
  listeners: Array<(event: T) => void>
  reconnectTimer: ReturnType<typeof setTimeout> | null
  reconnectAttempts: number
  connected: () => boolean
  setConnected: (v: boolean) => void
  reconnectCallbacks: Array<() => void>
}

function createWsInstance<T>(): WsInstance<T> {
  const [connected, setConnected] = createSignal(false)
  return {
    ws: null,
    listeners: [],
    reconnectTimer: null,
    reconnectAttempts: 0,
    connected,
    setConnected,
    reconnectCallbacks: [],
  }
}

const BASE_RECONNECT_MS = 1000
const MAX_RECONNECT_MS = 30000

function connectWs<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
) {
  if (inst.ws && inst.ws.readyState === WebSocket.OPEN) return

  try {
    inst.ws = new WebSocket(getUrl())

    inst.ws.onopen = () => {
      inst.setConnected(true)
      inst.reconnectAttempts = 0
      inst.reconnectCallbacks.forEach(cb => cb())
    }

    inst.ws.onmessage = (e) => {
      try {
        const event = parse(e.data)
        if (event) inst.listeners.forEach(l => l(event))
      } catch (err) {
        console.error('[WS] Failed to parse message:', err)
      }
    }

    inst.ws.onclose = () => {
      inst.setConnected(false)
      const delay = Math.min(BASE_RECONNECT_MS * Math.pow(2, inst.reconnectAttempts), MAX_RECONNECT_MS)
      inst.reconnectAttempts++
      inst.reconnectTimer = setTimeout(() => connectWs(inst, getUrl, parse), delay)
    }

    inst.ws.onerror = () => {}
  } catch (err) {
    console.error('[WS] Failed to connect:', err)
    const delay = Math.min(BASE_RECONNECT_MS * Math.pow(2, inst.reconnectAttempts), MAX_RECONNECT_MS)
    inst.reconnectAttempts++
    inst.reconnectTimer = setTimeout(() => connectWs(inst, getUrl, parse), delay)
  }
}

function useWsHook<T>(
  inst: WsInstance<T>,
  getUrl: () => string,
  parse: (raw: string) => T | null,
  onEvent: (event: T) => void,
  onReconnect?: () => void,
): { connected: () => boolean } {
  inst.listeners.push(onEvent)
  if (onReconnect) inst.reconnectCallbacks.push(onReconnect)

  if (!inst.ws || inst.ws.readyState === WebSocket.CLOSED) {
    inst.reconnectAttempts = 0
    connectWs(inst, getUrl, parse)
  }

  onCleanup(() => {
    inst.listeners = inst.listeners.filter(l => l !== onEvent)
    if (onReconnect) inst.reconnectCallbacks = inst.reconnectCallbacks.filter(cb => cb !== onReconnect)
    if (inst.listeners.length === 0 && inst.ws) {
      if (inst.reconnectTimer) clearTimeout(inst.reconnectTimer)
      inst.ws.close()
      inst.ws = null
    }
  })

  return { connected: inst.connected }
}

// ── Main WebSocket ─────────────────────────────────────────

const mainInst = createWsInstance<WsEvent>()

function getMainWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const token = localStorage.getItem('qd_token')
  const base = `${protocol}//${window.location.host}/ws`
  return token ? `${base}?token=${encodeURIComponent(token)}` : base
}

export function useWs(onEvent: (event: WsEvent) => void): { connected: () => boolean } {
  return useWsHook(mainInst, getMainWsUrl, (raw) => {
    try { return JSON.parse(raw) as WsEvent } catch { return null }
  }, onEvent)
}

// ── Kline WebSocket ────────────────────────────────────────

const klineInst = createWsInstance<KlineWsEvent>()

function getKlineWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}/ws/kline`
}

export function useKlineWs(
  onEvent: (event: KlineWsEvent) => void,
  onReconnect?: () => void,
): { connected: () => boolean } {
  return useWsHook(klineInst, getKlineWsUrl, (raw) => {
    try { return JSON.parse(raw) as KlineWsEvent } catch { return null }
  }, onEvent, onReconnect)
}

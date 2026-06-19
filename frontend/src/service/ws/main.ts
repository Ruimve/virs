import { createWsInstance, useWsHook } from '../../lib/ws'

// ── Event types (aligned with backend WsBroadcaster) ──────

export interface BotStatusEvent {
  type: 'bot_status'
  bot_id: string
  status: 'running' | 'stopped' | 'error'
}

export interface PositionEvent {
  type: 'position'
  bot_id: string
  side: string
  entry_price: number
  size: number
  action: 'opened' | 'closed'
}

export interface TradeEvent {
  type: 'trade'
  bot_id: string
  side: string
  price: number
  pnl: number
}

export interface PositionPnlEvent {
  type: 'position_pnl'
  bot_id: string
  symbol: string
  side: string
  entry_price: number
  position_size: number
  current_price: number
  unrealized_pnl: number
  total_pnl: number
  liquidation_price: number
}

export interface NotificationEvent {
  type: 'notification'
  level: 'info' | 'warning' | 'error'
  message: string
}

export interface PaperModeEvent {
  type: 'paper_mode'
  enabled: boolean
}

export type WsEvent =
  | BotStatusEvent
  | PositionEvent
  | TradeEvent
  | PositionPnlEvent
  | NotificationEvent
  | PaperModeEvent

// ── Main WebSocket ─────────────────────────────────────────

const mainInst = createWsInstance<WsEvent>()

function getMainWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const token = localStorage.getItem('qd_token')
  const base = `${protocol}//${window.location.host}/ws`
  return token ? `${base}?token=${encodeURIComponent(token)}` : base
}

const parseMainWs = (raw: string): WsEvent | null => {
  try {
    return JSON.parse(raw) as WsEvent
  } catch {
    return null
  }
}

export function useWs(onEvent: (event: WsEvent) => void): { connected: boolean } {
  return useWsHook(mainInst, getMainWsUrl, parseMainWs, onEvent)
}

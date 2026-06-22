import type { ReactNode } from 'react'

export const formatPnl = (pnl: number): ReactNode => {
  if (pnl > 0)
    return <span className="text-emerald-400 font-semibold font-mono">+{pnl.toFixed(4)}</span>
  if (pnl < 0) return <span className="text-red-400 font-semibold font-mono">{pnl.toFixed(4)}</span>
  return <span className="text-on-surface-tertiary font-mono">0.00</span>
}

export const formatPnlShort = (pnl: number): ReactNode => {
  if (pnl > 0)
    return <span className="text-emerald-400 font-semibold font-mono">+{pnl.toFixed(2)}</span>
  if (pnl < 0) return <span className="text-red-400 font-semibold font-mono">{pnl.toFixed(2)}</span>
  return <span className="text-on-surface-tertiary font-mono">0.00</span>
}

export const formatSmart = (value: number): string => {
  if (value === 0) return '0'
  if (value >= 1) return value.toFixed(4)
  if (value >= 0.001) return value.toFixed(6)
  if (value >= 0.000001) return value.toFixed(8)
  return value.toExponential(2)
}

export const actionLabel = (action: string | undefined) => {
  if (!action) return undefined

  const map: Record<string, string> = {
    open_long: '开多',
    open_short: '开空',
    close_position: '平仓',
    hold: '持有',
    reduce_position: '减仓',
    cancel_order: '取消订单',
    adjust_grid: '调整网格',
    pause_grid: '暂停网格',
    resume_grid: '恢复网格',
  }
  return map[action] || action
}

export const actionColor = (action: string | undefined) => {
  const defaultColor = 'bg-surface-2 text-on-surface-tertiary border-line-default'
  if (!action) return defaultColor

  const map: Record<string, string> = {
    open_long: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
    open_short: 'bg-red-500/10 text-red-400 border-red-500/20',
    close_position: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
    hold: defaultColor,
    reduce_position: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
    cancel_order: defaultColor,
    adjust_grid: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
    pause_grid: 'bg-red-500/10 text-red-400 border-red-500/20',
    resume_grid: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
  }
  return map[action] || defaultColor
}

export const confidenceColor = (conf: number) => {
  if (conf >= 0.7) return 'text-emerald-400'
  if (conf >= 0.4) return 'text-amber-400'
  return 'text-red-400'
}

export const tradeTypeLabel = (t: string) => {
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

export const tradeTypeColor = (t: string) => {
  if (t.startsWith('open_long') || t === 'close_short') return 'text-emerald-400'
  if (t.startsWith('open_short') || t === 'close_long') return 'text-red-400'
  if (t === 'stop_loss') return 'text-red-400'
  if (t === 'take_profit') return 'text-emerald-400'
  return 'text-on-surface-tertiary'
}

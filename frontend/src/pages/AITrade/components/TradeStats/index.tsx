import { memo, useMemo } from 'react'
import type { AutoTrade } from '../../../../service/types'

interface TradeStatsProps {
  trades: AutoTrade[]
  totalTrades: number
  winTrades: number
  lossTrades: number
}

interface Stats {
  winRate: number
  profitLossRatio: number
  maxDrawdown: number
  avgHoldTime: string
  maxWinStreak: number
  maxLossStreak: number
  totalFee: number
  netPnl: number
}

function computeStats(trades: AutoTrade[], totalTrades: number, winTrades: number): Stats {
  // 按时间正序排列用于计算回撤和连胜
  const sorted = [...trades].sort(
    (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
  )

  // 胜率
  const winRate = totalTrades > 0 ? (winTrades / totalTrades) * 100 : 0

  // 盈亏比 = 平均盈利 / 平均亏损
  const profits = sorted.filter((t) => t.pnl > 0)
  const losses = sorted.filter((t) => t.pnl < 0)
  const avgProfit = profits.length > 0 ? profits.reduce((s, t) => s + t.pnl, 0) / profits.length : 0
  const avgLoss =
    losses.length > 0 ? Math.abs(losses.reduce((s, t) => s + t.pnl, 0) / losses.length) : 0
  const profitLossRatio = avgLoss > 0 ? avgProfit / avgLoss : avgProfit > 0 ? Infinity : 0

  // 最大回撤（基于累计 PnL 峰值）
  let cumulative = 0
  let peak = 0
  let maxDrawdown = 0
  for (const t of sorted) {
    cumulative += t.pnl
    if (cumulative > peak) peak = cumulative
    const drawdown = peak - cumulative
    if (drawdown > maxDrawdown) maxDrawdown = drawdown
  }

  // 平均持仓时间（开仓到平仓的时间差）
  const closeTrades = sorted.filter((t) => t.trade_type.startsWith('close_'))
  const openTrades = sorted.filter((t) => t.trade_type.startsWith('open_'))
  let avgHoldMs = 0
  if (closeTrades.length > 0 && openTrades.length > 0) {
    let totalHoldMs = 0
    let pairCount = 0
    for (const close of closeTrades) {
      const closeTime = new Date(close.created_at).getTime()
      // 找到最近的同方向开仓
      const matchingOpen = [...openTrades]
        .reverse()
        .find(
          (o) =>
            new Date(o.created_at).getTime() <= closeTime &&
            o.trade_type.includes(close.trade_type.replace('close_', '')),
        )
      if (matchingOpen) {
        totalHoldMs += closeTime - new Date(matchingOpen.created_at).getTime()
        pairCount++
      }
    }
    avgHoldMs = pairCount > 0 ? totalHoldMs / pairCount : 0
  }
  const avgHoldTime = formatDuration(avgHoldMs)

  // 连胜/连亏
  let maxWinStreak = 0
  let maxLossStreak = 0
  let currentWin = 0
  let currentLoss = 0
  for (const t of sorted) {
    if (t.pnl > 0) {
      currentWin++
      currentLoss = 0
      if (currentWin > maxWinStreak) maxWinStreak = currentWin
    } else if (t.pnl < 0) {
      currentLoss++
      currentWin = 0
      if (currentLoss > maxLossStreak) maxLossStreak = currentLoss
    }
  }

  const totalFee = sorted.reduce((s, t) => s + (t.fee || 0), 0)
  const netPnl = sorted.reduce((s, t) => s + t.pnl, 0)

  return {
    winRate,
    profitLossRatio,
    maxDrawdown,
    avgHoldTime,
    maxWinStreak,
    maxLossStreak,
    totalFee,
    netPnl,
  }
}

function formatDuration(ms: number): string {
  if (ms <= 0) return '-'
  const seconds = Math.floor(ms / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  if (hours > 0) return `${hours}h${minutes % 60}m`
  if (minutes > 0) return `${minutes}m${seconds % 60}s`
  return `${seconds}s`
}

const TradeStats = ({ trades, totalTrades, winTrades, lossTrades }: TradeStatsProps) => {
  const stats = useMemo(
    () => computeStats(trades, totalTrades, winTrades),
    [trades, totalTrades, winTrades],
  )

  const items = [
    {
      label: '胜率',
      value: totalTrades > 0 ? `${stats.winRate.toFixed(1)}%` : '-',
      sub: `${winTrades}胜 / ${lossTrades}负`,
      color: stats.winRate >= 50 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '盈亏比',
      value:
        stats.profitLossRatio === Infinity
          ? '∞'
          : stats.profitLossRatio > 0
            ? stats.profitLossRatio.toFixed(2)
            : '-',
      color: stats.profitLossRatio >= 1 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '最大回撤',
      value: stats.maxDrawdown > 0 ? `-${stats.maxDrawdown.toFixed(2)}` : '-',
      color: 'text-red-400',
    },
    {
      label: '平均持仓',
      value: stats.avgHoldTime,
      color: 'text-on-surface',
    },
    {
      label: '连胜',
      value: stats.maxWinStreak > 0 ? `${stats.maxWinStreak}` : '-',
      color: 'text-emerald-400',
    },
    {
      label: '连亏',
      value: stats.maxLossStreak > 0 ? `${stats.maxLossStreak}` : '-',
      color: 'text-red-400',
    },
    {
      label: '净盈亏',
      value: stats.netPnl !== 0 ? stats.netPnl.toFixed(4) : '-',
      color: stats.netPnl >= 0 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '总手续费',
      value: stats.totalFee > 0 ? `-${stats.totalFee.toFixed(4)}` : '-',
      color: 'text-amber-400',
    },
  ]

  return (
    <div className="px-4 py-3 border-b border-line-subtle">
      <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
        历史交易统计
      </div>
      <div className="grid grid-cols-4 gap-3">
        {items.map((item) => (
          <div key={item.label}>
            <div className="text-[10px] text-on-surface-tertiary mb-0.5">{item.label}</div>
            <div className={`text-sm font-mono font-medium ${item.color}`}>{item.value}</div>
            {item.sub && <div className="text-[9px] text-on-surface-muted mt-0.5">{item.sub}</div>}
          </div>
        ))}
      </div>
    </div>
  )
}

export default memo(TradeStats)

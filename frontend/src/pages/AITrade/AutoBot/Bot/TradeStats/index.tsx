import { memo, useEffect, useState } from 'react'
import { getAutoStats, type AutoBotStats } from '@/service'

interface Props {
  botId: string
}

/** 格式化交易额：大数缩写 */
function formatVolume(v: number): string {
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(2)}M`
  if (v >= 1_000) return `${(v / 1_000).toFixed(2)}K`
  return v.toFixed(2)
}

const TradeStats = ({ botId }: Props) => {
  const [stats, setStats] = useState<AutoBotStats | null>(null)

  useEffect(() => {
    if (!botId) return
    let cancelled = false
    getAutoStats(botId)
      .then((res) => {
        if (cancelled) return
        if (res.success && res.data) setStats(res.data)
      })
      .catch((e) => console.error('Failed to load stats:', e))
    return () => {
      cancelled = true
    }
  }, [botId])

  const items = [
    {
      label: '累计盈亏',
      value: stats && stats.net_pnl !== 0 ? stats.net_pnl.toFixed(4) : '-',
      color: stats && stats.net_pnl >= 0 ? 'text-emerald-400' : 'text-red-400',
      highlight: true,
    },
    {
      label: '净盈亏',
      value: stats && stats.net_pnl_after_fee !== 0 ? stats.net_pnl_after_fee.toFixed(4) : '-',
      sub: '扣手续费',
      color: stats && stats.net_pnl_after_fee >= 0 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '胜率',
      value: stats && stats.total_trades > 0 ? `${stats.win_rate.toFixed(1)}%` : '-',
      sub: stats ? `${stats.win_trades}胜 / ${stats.loss_trades}负` : undefined,
      color: stats && stats.win_rate >= 50 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '盈亏比',
      value:
        stats && stats.profit_loss_ratio === Infinity
          ? '∞'
          : stats && stats.profit_loss_ratio > 0
            ? stats.profit_loss_ratio.toFixed(2)
            : '-',
      color: stats && stats.profit_loss_ratio >= 1 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '最大回撤',
      value: stats && stats.max_drawdown > 0 ? `-${stats.max_drawdown.toFixed(2)}` : '-',
      color: 'text-red-400',
    },
    {
      label: '平均盈亏',
      value: stats && stats.avg_pnl !== 0 ? stats.avg_pnl.toFixed(4) : '-',
      color: stats && stats.avg_pnl >= 0 ? 'text-emerald-400' : 'text-red-400',
    },
    {
      label: '最大盈利',
      value: stats && stats.max_profit > 0 ? `+${stats.max_profit.toFixed(4)}` : '-',
      color: 'text-emerald-400',
    },
    {
      label: '最大亏损',
      value: stats && stats.max_loss < 0 ? stats.max_loss.toFixed(4) : '-',
      color: 'text-red-400',
    },
    {
      label: '连胜',
      value: stats && stats.max_win_streak > 0 ? `${stats.max_win_streak}` : '-',
      color: 'text-emerald-400',
    },
    {
      label: '连亏',
      value: stats && stats.max_loss_streak > 0 ? `${stats.max_loss_streak}` : '-',
      color: 'text-red-400',
    },
    {
      label: '平均持仓',
      value: stats?.avg_hold_time ?? '-',
      color: 'text-on-surface',
    },
    {
      label: '总交易额',
      value: stats && stats.total_volume > 0 ? formatVolume(stats.total_volume) : '-',
      color: 'text-on-surface',
    },
    {
      label: '总手续费',
      value: stats && stats.total_fee > 0 ? `-${stats.total_fee.toFixed(4)}` : '-',
      color: 'text-amber-400',
    },
  ]

  return (
    <div className="px-4 py-3 border-b border-line-subtle">
      <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">
        历史交易统计
      </div>
      <div className="grid grid-cols-4 md:grid-cols-6 lg:grid-cols-7 gap-3">
        {items.map((item) => (
          <div key={item.label}>
            <div className="text-[10px] text-on-surface-tertiary mb-0.5">{item.label}</div>
            <div
              className={`font-mono font-medium ${item.highlight ? 'text-base' : 'text-sm'} ${item.color}`}
            >
              {item.value}
            </div>
            {item.sub && <div className="text-[9px] text-on-surface-muted mt-0.5">{item.sub}</div>}
          </div>
        ))}
      </div>
    </div>
  )
}

export default memo(TradeStats)

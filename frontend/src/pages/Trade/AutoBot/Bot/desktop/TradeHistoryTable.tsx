import { memo, useMemo } from 'react';
import type { AutoTrade } from '@/service/types';
import type { TradeHistoryTableProps } from './types';
import { formatCompact, formatSigned } from '../components/utils';

function formatTime(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return '—';
  return d.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export const TradeHistoryTable = memo(({ trades }: TradeHistoryTableProps) => {
  const displayTrades = useMemo(() => trades.slice(0, 50), [trades]);

  if (displayTrades.length === 0) {
    return (
      <div className="flex-1 flex flex-col bg-surface-1 border border-line-default rounded-lg overflow-hidden">
        <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle">
          <span className="text-xs font-semibold text-on-surface">交易记录</span>
          <span className="text-2xs text-on-surface-muted">最近 50 条</span>
        </div>
        <div className="flex-1 flex items-center justify-center py-8">
          <span className="text-xs text-on-surface-muted">暂无交易记录</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col bg-surface-1 border border-line-default rounded-lg overflow-hidden min-h-0">
      {}
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-xs font-semibold text-on-surface">交易记录</span>
        <span className="text-2xs text-on-surface-muted">最近 50 条</span>
      </div>

      {}
      <div className="flex-1 overflow-auto min-h-0">
        <table className="w-full text-2xs">
          <thead className="sticky top-0 bg-surface-2 z-10">
            <tr className="text-on-surface-tertiary">
              <th className="text-left font-medium px-3 py-1.5 whitespace-nowrap">时间</th>
              <th className="text-left font-medium px-3 py-1.5 whitespace-nowrap">方向</th>
              <th className="text-right font-medium px-3 py-1.5 whitespace-nowrap">价格</th>
              <th className="text-right font-medium px-3 py-1.5 whitespace-nowrap">数量</th>
              <th className="text-right font-medium px-3 py-1.5 whitespace-nowrap">盈亏</th>
            </tr>
          </thead>
          <tbody>
            {displayTrades.map((trade: AutoTrade) => {
              const isBuy = trade.open_side === 'buy';
              const isClosed = trade.status === 'closed';
              const pnlClass =
                trade.pnl > 0
                  ? 'text-success-text'
                  : trade.pnl < 0
                    ? 'text-danger-text'
                    : 'text-on-surface-tertiary';

              return (
                <tr
                  key={trade.id}
                  className="border-t border-line-subtle hover:bg-surface-2/50 transition-colors"
                >
                  <td className="px-3 py-1.5 text-on-surface-tertiary font-mono tabular-nums whitespace-nowrap">
                    {formatTime(trade.opened_at)}
                  </td>
                  <td className="px-3 py-1.5 whitespace-nowrap">
                    <span
                      className={`inline-block px-1.5 py-0.5 rounded text-2xs font-semibold ${
                        isBuy ? 'bg-success-bg text-success-text' : 'bg-danger-bg text-danger-text'
                      }`}
                    >
                      {isBuy ? 'BUY' : 'SELL'}
                    </span>
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono tabular-nums text-on-surface whitespace-nowrap">
                    {formatCompact(trade.open_price)}
                  </td>
                  <td className="px-3 py-1.5 text-right font-mono tabular-nums text-on-surface-secondary whitespace-nowrap">
                    {trade.open_quantity}
                  </td>
                  <td
                    className={`px-3 py-1.5 text-right font-mono tabular-nums font-semibold whitespace-nowrap ${pnlClass}`}
                  >
                    {isClosed ? formatSigned(trade.pnl) : '—'}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
});

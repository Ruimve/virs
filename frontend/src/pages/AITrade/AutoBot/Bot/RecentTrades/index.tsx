import { memo } from 'react';
import type { AutoTrade } from '@/service';
import { formatTime, formatSmart } from '../../../components/utils/utils';

interface Props {
  trades: AutoTrade[];
}

const pnlColor = (v: number) =>
  v > 0 ? 'text-emerald-400' : v < 0 ? 'text-red-400' : 'text-on-surface';

const RecentTrades = ({ trades }: Props) => {
  const recent = trades.slice(0, 10);

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">
          最近交易
        </span>
        <span className="text-[11px] font-mono tabular-nums text-on-surface-muted">
          {trades.length}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {recent.length === 0 ? (
          <div className="text-center py-6 text-sm text-on-surface-tertiary">暂无交易</div>
        ) : (
          <div className="divide-y divide-line-subtle">
            {recent.map((trade) => (
              <div key={trade.id} className="px-3 py-2">
                {/* 第一行：方向 + 开平仓价 + 时间 */}
                <div className="flex items-center gap-2 mb-1">
                  <span
                    className={`text-[11px] font-medium px-1.5 py-0.5 rounded shrink-0 ${trade.open_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                  >
                    {trade.open_side === 'buy' ? '多' : '空'}
                  </span>
                  <span className="text-sm font-mono tabular-nums text-on-surface">
                    {trade.open_price.toFixed(2)}
                  </span>
                  <span className="text-[11px] text-on-surface-muted">→</span>
                  <span className="text-sm font-mono tabular-nums text-on-surface">
                    {trade.close_price?.toFixed(2) ?? '-'}
                  </span>
                  <span className="text-[11px] text-on-surface-muted ml-auto font-mono tabular-nums">
                    {formatTime(trade.closed_at || trade.opened_at)}
                  </span>
                </div>
                {/* 第二行：数量 + 盈亏 */}
                <div className="flex items-center gap-3 text-[11px]">
                  <span className="text-on-surface-tertiary">
                    量{' '}
                    <span className="font-mono tabular-nums text-on-surface">
                      {formatSmart(trade.open_quantity)}
                    </span>
                  </span>
                  {trade.status === 'closed' && (
                    <span className={pnlColor(trade.pnl)}>
                      盈亏{' '}
                      <span className="font-mono tabular-nums">
                        {trade.pnl >= 0 ? '+' : ''}
                        {trade.pnl.toFixed(4)}
                      </span>
                    </span>
                  )}
                  {trade.status === 'open' && <span className="text-amber-400">持仓中</span>}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default memo(RecentTrades);

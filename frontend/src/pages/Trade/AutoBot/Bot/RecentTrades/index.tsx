import { memo } from 'react';
import type { AutoTrade } from '@/service';
import PanelSection from '@/components/PanelSection';
import Badge from '@/components/Badge';
import { formatTime, formatSmart, pnlColor } from '../../../components/utils/utils';

interface Props {
  trades: AutoTrade[];
}

const RecentTrades = ({ trades }: Props) => {
  const recent = trades.slice(0, 10);

  return (
    <PanelSection
      title="最近交易"
      count={trades.length}
      empty={recent.length === 0}
      emptyText="暂无交易"
    >
      {recent.map((trade) => (
        <div key={trade.id} className="px-3 py-2">
          {/* First row: direction + prices + time */}
          <div className="flex items-center gap-2 mb-1">
            <Badge variant={trade.open_side === 'buy' ? 'success' : 'danger'} size="sm">
              {trade.open_side === 'buy' ? '多' : '空'}
            </Badge>
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
          {/* Second row: qty + SL/TP + PnL */}
          <div className="flex items-center gap-3 text-[11px] flex-wrap">
            <span className="text-on-surface-tertiary">
              量{' '}
              <span className="font-mono tabular-nums text-on-surface">
                {formatSmart(trade.open_quantity)}
              </span>
            </span>
            {trade.stop_loss > 0 && (
              <span className="text-danger-text/80">
                SL <span className="font-mono tabular-nums">{trade.stop_loss.toFixed(2)}</span>
              </span>
            )}
            {trade.take_profit > 0 && (
              <span className="text-success-text/80">
                TP <span className="font-mono tabular-nums">{trade.take_profit.toFixed(2)}</span>
              </span>
            )}
            {trade.status === 'closed' && (
              <span className={`ml-auto ${pnlColor(trade.pnl)}`}>
                盈亏{' '}
                <span className="font-mono tabular-nums">
                  {trade.pnl >= 0 ? '+' : ''}
                  {trade.pnl.toFixed(4)}
                </span>
              </span>
            )}
            {trade.status === 'open' && <span className="text-warning-text ml-auto">持仓中</span>}
          </div>
        </div>
      ))}
    </PanelSection>
  );
};

export default memo(RecentTrades);

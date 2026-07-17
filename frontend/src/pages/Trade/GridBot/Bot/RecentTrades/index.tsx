import { memo, useEffect, useState } from 'react';
import { getGridTrades, type GridTrade } from '@/service';
import { Panel } from '@/components/Panel';
import { Badge } from '@/components/Badge';
import { formatPnlShort, pnlColor } from '../../../components/utils/utils';

interface Props {
  botId: string;
}

export const RecentTrades = memo(({ botId }: Props) => {
  const [trades, setTrades] = useState<GridTrade[]>([]);
  const [, setLoading] = useState(true);

  useEffect(() => {
    if (!botId) return;
    let cancelled = false;
    setLoading(true);
    getGridTrades(botId, 1, 5)
      .then((res) => {
        if (cancelled) return;
        if (res.success && res.data) setTrades(res.data.trades || []);
      })
      .catch((e) => console.error('Failed to load grid trades:', e))
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [botId]);

  const recent = trades.slice(0, 5);

  return (
    <Panel title="最近成交" count={trades.length} empty={recent.length === 0} emptyText="暂无成交">
      {recent.map((t) => (
        <div key={t.id} className="px-3 py-2">
          <div className="flex items-center gap-2 mb-1">
            <Badge variant={t.open_side === 'buy' ? 'success' : 'danger'} size="sm">
              {t.open_side === 'buy' ? '买入' : '卖出'} L{t.grid_level}
            </Badge>
            <span className="text-caption text-on-surface-muted ml-auto font-mono tabular-nums">
              {new Date(t.opened_at).toLocaleTimeString('zh-CN', {
                hour: '2-digit',
                minute: '2-digit',
              })}
            </span>
          </div>
          <div className="flex items-center justify-between text-sm">
            <span className="text-on-surface font-mono tabular-nums">
              {t.open_quantity.toFixed(4)} @ {t.open_price.toFixed(2)}
            </span>
            {t.status === 'closed' && (
              <span className={`font-mono tabular-nums ${pnlColor(t.pnl)}`}>
                {formatPnlShort(t.pnl)}
              </span>
            )}
          </div>
        </div>
      ))}
    </Panel>
  );
});

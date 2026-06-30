import { memo, useEffect, useState } from 'react';
import { getGridTrades, type GridTrade } from '@/service';
import { formatPnlShort } from '../../../components/utils/utils';

interface Props {
  botId: string;
}

const pnlColor = (v: number) =>
  v > 0 ? 'text-success-text' : v < 0 ? 'text-danger-text' : 'text-on-surface';

/** 网格机器人最近成交卡片（右侧栏） */
const RecentTrades = ({ botId }: Props) => {
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
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">
          最近成交
        </span>
        <span className="text-[11px] font-mono tabular-nums text-on-surface-muted">
          {trades.length}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {recent.length === 0 ? (
          <div className="text-center py-6 text-sm text-on-surface-tertiary">暂无成交</div>
        ) : (
          <div className="divide-y divide-line-subtle">
            {recent.map((t) => (
              <div key={t.id} className="px-3 py-2">
                <div className="flex items-center gap-2 mb-1">
                  <span
                    className={`text-[11px] font-medium px-1.5 py-0.5 rounded shrink-0 ${t.open_side === 'buy' ? 'bg-success-bg text-success-text' : 'bg-danger-bg text-danger-text'}`}
                  >
                    {t.open_side === 'buy' ? '买入' : '卖出'} L{t.grid_level}
                  </span>
                  <span className="text-[11px] text-on-surface-muted ml-auto font-mono tabular-nums">
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
          </div>
        )}
      </div>
    </div>
  );
};

export default memo(RecentTrades);

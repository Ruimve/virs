import { getGridTrades, type GridTrade } from '@/service';
import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { TradeLoading } from '@/components/Transition/Icon';
import { Badge } from '@/components/Badge';
import { StateFeedback } from '@/components/StateFeedback';
import { Pagination } from '@/components/Pagination';
import { formatPnlShort } from '../../components/utils/utils';

const PAGE_SIZE = 20;

const Trades = () => {
  const { botId } = useParams<{ botId: string }>();
  const [trades, setTrades] = useState<GridTrade[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);

  const loadTrades = useCallback(
    async (p: number) => {
      if (!botId) return;
      setLoading(true);
      try {
        const res = await getGridTrades(botId, p, PAGE_SIZE);
        if (res.success && res.data) {
          setTrades(res.data.trades || []);
          setTotal(res.data.total || 0);
          setPage(p);
        }
      } finally {
        setLoading(false);
      }
    },
    [botId],
  );

  useEffect(() => {
    loadTrades(1);
  }, [loadTrades]);

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="h-full overflow-y-auto max-w-7xl mx-auto px-4 md:px-8 py-6">
      {trades.length > 0 ? (
        <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-on-surface-tertiary border-b border-line-subtle bg-base-secondary">
                <th className="text-right px-4 py-2.5 font-medium">层级</th>
                <th className="text-left px-3 py-2.5 font-medium">开仓</th>
                <th className="text-right px-3 py-2.5 font-medium">开仓价</th>
                <th className="text-right px-3 py-2.5 font-medium">开仓量</th>
                <th className="text-left px-3 py-2.5 font-medium">平仓</th>
                <th className="text-right px-3 py-2.5 font-medium">平仓价</th>
                <th className="text-right px-3 py-2.5 font-medium">平仓量</th>
                <th className="text-right px-3 py-2.5 font-medium">盈亏</th>
                <th className="text-left px-3 py-2.5 font-medium">状态</th>
              </tr>
            </thead>
            <tbody>
              {trades.map((t) => (
                <tr key={t.id} className="border-b border-line-subtle">
                  <td className="px-4 py-2 text-on-surface-secondary font-mono text-right">
                    {t.grid_level}
                  </td>
                  <td className="px-3 py-2">
                    <Badge variant={t.open_side === 'buy' ? 'success' : 'danger'}>
                      {t.open_side === 'buy' ? '买入' : '卖出'}
                    </Badge>
                  </td>
                  <td className="px-3 py-2 text-on-surface font-mono text-right">
                    {t.open_price.toFixed(2)}
                  </td>
                  <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                    {t.open_quantity.toFixed(4)}
                  </td>
                  <td className="px-3 py-2">
                    {t.close_side ? (
                      <Badge variant={t.close_side === 'buy' ? 'success' : 'danger'}>
                        {t.close_side === 'buy' ? '买入' : '卖出'}
                      </Badge>
                    ) : (
                      <span className="text-on-surface-faint">-</span>
                    )}
                  </td>
                  <td className="px-3 py-2 text-on-surface font-mono text-right">
                    {t.close_price?.toFixed(2) || '-'}
                  </td>
                  <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                    {t.close_quantity?.toFixed(4) || '-'}
                  </td>
                  <td className="px-3 py-2 text-right">{formatPnlShort(t.pnl)}</td>
                  <td className="px-3 py-2">
                    <Badge
                      variant={
                        t.status === 'closed' ? 'info' : t.status === 'open' ? 'warning' : 'neutral'
                      }
                    >
                      {t.status === 'closed' ? '已平' : t.status === 'open' ? '持仓' : t.status}
                    </Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {/* 分页 */}
          <Pagination
            total={total}
            page={page}
            totalPages={totalPages}
            loading={loading}
            onPrev={() => loadTrades(page - 1)}
            onNext={() => loadTrades(page + 1)}
          />
        </div>
      ) : loading ? (
        <StateFeedback type="loading" text="交易记录加载中" icon={<TradeLoading size={40} />} />
      ) : (
        <StateFeedback type="empty" text="暂无交易记录" />
      )}
    </div>
  );
};

export default Trades;

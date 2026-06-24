import { getGridTrades, type GridTrade } from '@/service';
import { memo, useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
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
    <div className="max-w-7xl mx-auto px-4 md:px-8 py-6">
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
                    <span
                      className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.open_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                    >
                      {t.open_side === 'buy' ? '买入' : '卖出'}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-on-surface font-mono text-right">
                    {t.open_price.toFixed(2)}
                  </td>
                  <td className="px-3 py-2 text-on-surface-secondary font-mono text-right">
                    {t.open_quantity.toFixed(4)}
                  </td>
                  <td className="px-3 py-2">
                    {t.close_side ? (
                      <span
                        className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${t.close_side === 'buy' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}
                      >
                        {t.close_side === 'buy' ? '买入' : '卖出'}
                      </span>
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
                    <span
                      className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium ${
                        t.status === 'closed'
                          ? 'bg-emerald-500/10 text-emerald-400'
                          : t.status === 'open'
                            ? 'bg-blue-500/10 text-blue-400'
                            : 'bg-surface-2 text-on-surface-tertiary'
                      }`}
                    >
                      {t.status === 'closed' ? '已平' : t.status === 'open' ? '持仓' : t.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {/* 分页 */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between px-5 py-3 border-t border-line-subtle text-xs">
              <span className="text-on-surface-tertiary">
                共 {total} 条 · 第 {page}/{totalPages} 页
              </span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => loadTrades(page - 1)}
                  disabled={page <= 1 || loading}
                  className="px-2 py-1 rounded border border-line-default text-on-surface-secondary disabled:opacity-40 hover:bg-surface-2"
                >
                  上一页
                </button>
                <button
                  onClick={() => loadTrades(page + 1)}
                  disabled={page >= totalPages || loading}
                  className="px-2 py-1 rounded border border-line-default text-on-surface-secondary disabled:opacity-40 hover:bg-surface-2"
                >
                  下一页
                </button>
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="text-center py-12 text-on-surface-tertiary text-xs">
          {loading ? '加载中...' : '暂无交易记录'}
        </div>
      )}
    </div>
  );
};

export default memo(Trades);

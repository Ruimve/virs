import { useCallback, useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getAutoTrades, type AutoTrade } from '@/service';
import { TradeLoading } from '../../components/Transition';
import { Badge, type BadgeVariant } from '@/components/Badge';
import { StateFeedback } from '@/components/StateFeedback';

const PAGE_SIZE = 20;

const closeReasonMap: Record<string, { variant: BadgeVariant; label: string }> = {
  stop_loss: { variant: 'danger', label: '止损' },
  take_profit: { variant: 'success', label: '止盈' },
  position_timeout: { variant: 'warning', label: '超时' },
  llm_decision: { variant: 'info', label: 'LLM平仓' },
};

const formatShortTime = (iso: string) => {
  const d = new Date(iso);
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  return `${mm}/${dd} ${hh}:${mi}`;
};

const Trades = () => {
  const { botId } = useParams<{ botId: string }>();
  const [trades, setTrades] = useState<AutoTrade[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  const loadTrades = useCallback(
    async (p: number, append: boolean) => {
      if (!botId) return;
      setLoading(true);
      try {
        const res = await getAutoTrades(botId, p, PAGE_SIZE);
        if (res.success && res.data) {
          const items = res.data.trades || [];
          setTrades((prev) => (append ? [...prev, ...items] : items));
          setTotal(res.data.total || 0);
          setPage(p);
        }
      } catch (e) {
        console.error('Failed to load trades:', e);
      } finally {
        setLoading(false);
      }
    },
    [botId],
  );

  useEffect(() => {
    loadTrades(1, false);
  }, [loadTrades]);

  const hasMore = trades.length < total;

  useEffect(() => {
    const el = sentinelRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        if (entry.isIntersecting && !loading && hasMore) {
          loadTrades(page + 1, true);
        }
      },
      { rootMargin: '200px 0px' },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [loading, hasMore, page, loadTrades]);

  if (trades.length === 0 && loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <StateFeedback type="loading" text="交易记录加载中" icon={<TradeLoading size={40} />} />
      </div>
    );
  }

  if (trades.length === 0) {
    return (
      <div className="h-full flex items-center justify-center">
        <StateFeedback type="empty" text="暂无交易记录" />
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto px-4 md:px-8 py-6">
      {/* Title row */}
      <div className="flex items-end justify-between mb-4">
        <div>
          <div className="text-sm font-semibold text-on-surface">交易记录</div>
          <div className="text-2xs text-on-surface-tertiary font-mono mt-0.5">共 {total} 条</div>
        </div>
      </div>

      {/* Desktop: table layout */}
      <div className="hidden md:block bg-surface-1 rounded-xl border border-line-default overflow-hidden">
        <table className="w-full">
          <thead>
            <tr className="bg-surface-2 border-b border-line-default">
              <th className="text-left text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                时间
              </th>
              <th className="text-left text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                方向
              </th>
              <th className="text-right text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                开仓价
              </th>
              <th className="text-right text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                数量
              </th>
              <th className="text-right text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                平仓价
              </th>
              <th className="text-right text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                手续费
              </th>
              <th className="text-right text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                盈亏
              </th>
              <th className="text-right text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                收益率
              </th>
              <th className="text-left text-2xs font-medium text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                状态
              </th>
            </tr>
          </thead>
          <tbody>
            {trades.map((t, idx) => {
              const isClosed = t.status === 'closed';
              const totalFee = t.open_fee + t.close_fee;
              const isOpenBuy = t.open_side === 'buy';
              const pnlPositive = t.pnl > 0;
              const pnlColor = pnlPositive ? 'text-success-text' : 'text-danger-text';
              const statusInfo =
                t.status === 'open'
                  ? { variant: 'warning' as const, label: '持仓中' }
                  : (closeReasonMap[t.close_reason ?? ''] ?? {
                      variant: 'warning' as const,
                      label: '已平仓',
                    });

              return (
                <tr
                  key={`${t.id}-${idx}`}
                  className="border-t border-line-subtle hover:bg-surface-2/50"
                >
                  <td className="text-2xs font-mono tabular-nums text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                    {formatShortTime(t.opened_at)}
                  </td>
                  <td className="px-4 py-2.5 whitespace-nowrap">
                    <span
                      className={`text-2xs font-mono font-semibold ${isOpenBuy ? 'text-success-text' : 'text-danger-text'}`}
                    >
                      {isOpenBuy ? 'BUY' : 'SELL'}
                    </span>
                  </td>
                  <td className="text-right text-xs font-mono font-semibold tabular-nums text-on-surface px-4 py-2.5 whitespace-nowrap">
                    {t.open_price.toFixed(2)}
                  </td>
                  <td className="text-right text-2xs font-mono tabular-nums text-on-surface-secondary px-4 py-2.5 whitespace-nowrap">
                    {t.open_quantity.toFixed(4)}
                  </td>
                  <td className="text-right text-xs font-mono tabular-nums text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                    {isClosed && t.close_price != null ? t.close_price.toFixed(2) : '—'}
                  </td>
                  <td className="text-right text-2xs font-mono tabular-nums text-on-surface-tertiary px-4 py-2.5 whitespace-nowrap">
                    {totalFee > 0 ? totalFee.toFixed(4) : '—'}
                  </td>
                  <td
                    className={`text-right text-xs font-mono font-bold tabular-nums px-4 py-2.5 whitespace-nowrap ${isClosed ? pnlColor : 'text-on-surface-tertiary'}`}
                  >
                    {isClosed && t.pnl !== 0 ? `${pnlPositive ? '+' : ''}${t.pnl.toFixed(4)}` : '—'}
                  </td>
                  <td
                    className={`text-right text-2xs font-mono tabular-nums px-4 py-2.5 whitespace-nowrap ${isClosed ? pnlColor : 'text-on-surface-tertiary'}`}
                  >
                    {isClosed && t.pnl_pct !== 0
                      ? `${pnlPositive ? '+' : ''}${t.pnl_pct.toFixed(2)}%`
                      : '—'}
                  </td>
                  <td className="px-4 py-2.5 whitespace-nowrap">
                    <Badge variant={statusInfo.variant} size="sm">
                      {statusInfo.label}
                    </Badge>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Mobile: card layout */}
      <div className="md:hidden space-y-2.5">
        {trades.map((t, idx) => {
          const isClosed = t.status === 'closed';
          const totalFee = t.open_fee + t.close_fee;
          const isOpenBuy = t.open_side === 'buy';
          const pnlPositive = t.pnl > 0;
          const pnlColor = pnlPositive ? 'text-success-text' : 'text-danger-text';
          const statusInfo =
            t.status === 'open'
              ? { variant: 'warning' as const, label: '持仓中' }
              : (closeReasonMap[t.close_reason ?? ''] ?? {
                  variant: 'warning' as const,
                  label: '已平仓',
                });

          return (
            <div
              key={`${t.id}-${idx}`}
              className="bg-surface-1 rounded-xl border border-line-default p-3.5"
            >
              {/* Row 1: status badge + direction + time */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Badge variant={statusInfo.variant} size="sm">
                    {statusInfo.label}
                  </Badge>
                  <span
                    className={`text-2xs font-mono font-semibold ${isOpenBuy ? 'text-success-text' : 'text-danger-text'}`}
                  >
                    {isOpenBuy ? '开多' : '开空'}
                  </span>
                </div>
                <span className="text-2xs font-mono tabular-nums text-on-surface-tertiary">
                  {formatShortTime(t.opened_at)}
                </span>
              </div>

              {/* Divider */}
              <div className="h-px bg-line-subtle my-2.5" />

              {/* Row 2: price → close price + quantity | PnL + PnL% */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5 min-w-0">
                  <span className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                    {t.open_price.toFixed(2)}
                  </span>
                  {isClosed && t.close_price != null && (
                    <>
                      <span className="text-2xs text-on-surface-muted">→</span>
                      <span className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                        {t.close_price.toFixed(2)}
                      </span>
                    </>
                  )}
                  <span className="text-2xs font-mono tabular-nums text-on-surface-tertiary shrink-0">
                    {t.open_quantity.toFixed(4)}
                  </span>
                </div>
                {isClosed && t.pnl !== 0 && (
                  <div className="text-right shrink-0">
                    <div className={`text-xs font-mono font-bold tabular-nums ${pnlColor}`}>
                      {pnlPositive ? '+' : ''}
                      {t.pnl.toFixed(4)}
                    </div>
                    {t.pnl_pct !== 0 && (
                      <div className={`text-2xs font-mono tabular-nums ${pnlColor}`}>
                        {pnlPositive ? '+' : ''}
                        {t.pnl_pct.toFixed(2)}%
                      </div>
                    )}
                  </div>
                )}
              </div>

              {/* Row 3: fee */}
              {totalFee > 0 && (
                <div className="mt-1.5">
                  <span className="text-2xs font-mono tabular-nums text-warning-text">
                    手续费 {totalFee.toFixed(4)}
                  </span>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Infinite scroll sentinel */}
      <div
        ref={sentinelRef}
        className="h-12 flex items-center justify-center text-xs text-on-surface-tertiary"
      >
        {loading ? '加载中...' : hasMore ? '上拉加载更多' : trades.length > 0 ? '已加载全部' : ''}
      </div>
    </div>
  );
};

export default Trades;

import { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getAutoTrades, type AutoTrade } from '@/service';
import { TradeLoading } from '@/components/Transition/Icon';
import { Badge } from '@/components/Badge';
import { StateFeedback } from '@/components/StateFeedback';
import { Pagination } from '@/components/Pagination';
import { formatPnl } from '../../components/utils/utils';

const PAGE_SIZE = 20;

const Trades = () => {
  const { botId } = useParams<{ botId: string }>();
  const [trades, setTrades] = useState<AutoTrade[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);

  const loadTrades = useCallback(
    async (p: number) => {
      if (!botId) return;
      setLoading(true);
      try {
        const res = await getAutoTrades(botId, p, PAGE_SIZE);
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
    <div className="h-full overflow-y-auto max-w-5xl mx-auto px-4 md:px-8 py-6">
      {trades.length > 0 ? (
        <div className="bg-surface-1 rounded-xl border border-line-default shadow-sm overflow-hidden">
          <div className="divide-y divide-line-subtle">
            {trades.map((t, idx) => {
              const isClosed = t.status === 'closed';
              const totalFee = t.open_fee + t.close_fee;
              return (
                <div
                  key={`${t.id}-${idx}`}
                  className="flex items-center justify-between px-5 py-3 hover:bg-surface-2/50"
                >
                  <div className="flex items-center gap-3">
                    <Badge
                      variant={
                        t.status === 'open'
                          ? 'warning'
                          : t.close_reason === 'stop_loss'
                            ? 'danger'
                            : t.close_reason === 'take_profit'
                              ? 'success'
                              : t.close_reason === 'llm_decision'
                                ? 'info'
                                : 'warning'
                      }
                    >
                      {t.status === 'open'
                        ? '持仓中'
                        : t.close_reason === 'stop_loss'
                          ? '止损'
                          : t.close_reason === 'take_profit'
                            ? '止盈'
                            : t.close_reason === 'position_timeout'
                              ? '超时'
                              : t.close_reason === 'llm_decision'
                                ? 'LLM平仓'
                                : '已平仓'}
                    </Badge>
                    <div>
                      <div className="text-xs text-on-surface font-mono">
                        {t.open_side === 'buy' ? '开多' : '开空'} {t.open_quantity.toFixed(6)} @{' '}
                        {t.open_price.toFixed(2)}
                        {isClosed && t.close_side && (
                          <span className="text-on-surface-secondary ml-2">
                            → {t.close_side === 'buy' ? '平空' : '平多'}{' '}
                            {t.close_quantity?.toFixed(6)} @ {t.close_price?.toFixed(2)}
                          </span>
                        )}
                      </div>
                      <div className="text-2xs text-on-surface-tertiary mt-0.5">
                        {new Date(t.opened_at).toLocaleString('zh-CN')}
                        {isClosed && t.closed_at && (
                          <span className="ml-1">
                            {' '}
                            → {new Date(t.closed_at).toLocaleString('zh-CN')}
                          </span>
                        )}
                        {totalFee > 0 && (
                          <span className="text-warning-text ml-2 font-mono">
                            手续费 {totalFee.toFixed(4)}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                  {isClosed && t.pnl !== 0 && (
                    <div className="text-right">
                      {formatPnl(t.pnl)}
                      {t.pnl_pct !== 0 && (
                        <div className="text-2xs text-on-surface-tertiary font-mono">
                          {t.pnl_pct.toFixed(2)} %
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          {}
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

import { formatPnl } from '../components/shared';

interface AutoBot {
  current_side: string | null;
  entry_price: number;
  position_size: number;
  stop_loss: number;
  take_profit: number;
  unrealized_pnl: number;
  total_pnl: number;
  total_trades: number;
  win_trades: number;
  loss_trades: number;
  leverage: number;
  decide_interval_secs: number;
}

interface PositionStatsProps {
  bot: AutoBot;
}

export default function PositionStats({ bot }: PositionStatsProps) {
  const b = bot;

  return (
    <div className="px-4 pb-4 grid grid-cols-1 sm:grid-cols-3 gap-3">
      {/* Position card */}
      <div className="bg-surface-1 rounded-xl border border-line-default p-4 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">仓位</div>
        {b.current_side && b.current_side !== 'none' ? (
          <div className="space-y-1.5">
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">方向</span>
              <span className={b.current_side === 'long' ? 'text-emerald-400 font-medium' : 'text-red-400 font-medium'}>
                {b.current_side === 'long' ? '做多' : '做空'}
              </span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">入场价</span>
              <span className="text-on-surface font-mono">{b.entry_price.toFixed(2)}</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">持仓量</span>
              <span className="text-on-surface font-mono">{b.position_size.toFixed(6)}</span>
            </div>
            <div className="flex justify-between text-xs pt-1.5 border-t border-line-subtle">
              <span className="text-on-surface-tertiary">未实现盈亏</span>
              {formatPnl(b.unrealized_pnl)}
            </div>
          </div>
        ) : (
          <div className="text-xs text-on-surface-tertiary">无持仓</div>
        )}
      </div>

      {/* Stats card */}
      <div className="bg-surface-1 rounded-xl border border-line-default p-4 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">统计</div>
        <div className="space-y-1.5">
          <div className="flex justify-between text-xs">
            <span className="text-on-surface-tertiary">累计盈亏</span>
            {formatPnl(b.total_pnl)}
          </div>
          <div className="flex justify-between text-xs">
            <span className="text-on-surface-tertiary">交易次数</span>
            <span className="text-on-surface font-mono">{b.total_trades}</span>
          </div>
          <div className="flex justify-between text-xs">
            <span className="text-on-surface-tertiary">胜率</span>
            <span className="text-on-surface font-mono">
              {b.total_trades > 0 ? ((b.win_trades / b.total_trades) * 100).toFixed(1) + '%' : '-'}
            </span>
          </div>
        </div>
      </div>

      {/* Config card */}
      <div className="bg-surface-1 rounded-xl border border-line-default p-4 shadow-sm">
        <div className="text-[10px] text-on-surface-tertiary uppercase tracking-wider mb-2">配置</div>
        <div className="space-y-1.5">
          <div className="flex justify-between text-xs">
            <span className="text-on-surface-tertiary">杠杆</span>
            <span className="text-on-surface font-mono">{b.leverage}x</span>
          </div>
          <div className="flex justify-between text-xs">
            <span className="text-on-surface-tertiary">决策周期</span>
            <span className="text-on-surface font-mono">{b.decide_interval_secs}s</span>
          </div>
        </div>
      </div>
    </div>
  );
}

import { memo, type ReactNode } from 'react';
import type { AnalysisLog, AutoBot, StrategyDetail } from '@/service/types';
import { Badge } from '@/components/Badge';
import { actionLabel, actionVariant } from '../../../components/utils/utils';
import type { StrategyBlockProps } from './types';
import { formatCompact, formatSigned, formatRelativeTime, formatDuration } from './utils';


function formatInterval(secs: number): string {
  if (secs <= 0) return '';
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
}


function extractDecision(log: AnalysisLog | null) {
  if (!log?.result) return null;
  if (log.result.decision) return log.result.decision;
  if (log.result.action) {
    return {
      action: log.result.action,
      reason: log.result.reason ?? '',
      confidence: log.result.confidence ?? 0,
    };
  }
  return null;
}


function resolveStrategyName(bot: AutoBot, strategy: StrategyDetail | null | undefined): string {
  if (strategy?.name) return strategy.name;
  if (bot.strategy_file) return bot.strategy_file;
  return '未绑定策略';
}


function deriveDirection(action: string | undefined): string {
  if (action === 'open_long') return '做多';
  if (action === 'open_short') return '做空';
  if (action === 'reduce_position') return '减仓';
  return '';
}


function MetaItem({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-1 px-2 first:pl-0 [&:not(:last-child)]:border-r border-line-subtle">
      {children}
    </div>
  );
}


export const StrategyBlock = memo(
  ({
    bot,
    strategy,
    decision,
    stats,
    totalPnl,
    totalPnlPct,
    decideIntervalSecs,
  }: StrategyBlockProps) => {
    const dec = extractDecision(decision);
    const strategyName = resolveStrategyName(bot, strategy);
    const strategyDesc = strategy?.description;
    const direction = deriveDirection(dec?.action);
    const reason = dec?.reason || bot.ai_analysis || '';
    const confidence = dec?.confidence;
    const model = decision?.llm_model;
    const regime = bot.market_regime;

    const pnlClass =
      totalPnl > 0 ? 'text-success-text' : totalPnl < 0 ? 'text-danger-text' : 'text-on-surface';

    const interval = formatInterval(decideIntervalSecs);
    const lastDecisionTime = formatRelativeTime(decision?.created_at);
    const runningDuration = formatDuration(bot.created_at);

    return (
      <div className="border border-line-default rounded-lg p-2.5 flex flex-col gap-2 bg-base-secondary">
        {}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <span
              className="inline-block w-1 h-2.5 bg-accent"
              style={{ animation: 'virs-cursor-blink 1.1s steps(2) infinite' }}
            />
            <span className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider">
              AI 自主策略
            </span>
          </div>
          <span className="text-2xs text-on-surface-muted font-mono">
            {model ?? ''}
            {confidence != null && !isNaN(confidence) ? ` · conf ${confidence.toFixed(2)}` : ''}
          </span>
        </div>

        {}
        <div className="flex items-baseline justify-between gap-2">
          <div className="flex items-baseline gap-1.5">
            <span className="text-sm font-bold leading-tight">{strategyName}</span>
            {direction && (
              <span className="text-xs font-semibold text-on-surface-secondary">{direction}</span>
            )}
          </div>
          <div className="flex flex-col items-end gap-0">
            <span
              className={`text-base font-bold font-mono tabular-nums leading-none tracking-tight ${pnlClass}`}
            >
              {formatSigned(totalPnl)}
            </span>
            <span className={`text-2xs font-semibold font-mono tabular-nums ${pnlClass}`}>
              {totalPnlPct > 0 ? '+' : ''}
              {totalPnlPct.toFixed(2)}%
            </span>
          </div>
        </div>

        {}
        {(reason || strategyDesc) && (
          <div className="border-l-2 border-accent-muted pl-2 text-2xs text-on-surface-secondary leading-relaxed line-clamp-3">
            {reason || strategyDesc}
          </div>
        )}

        {}
        <div className="flex items-center flex-wrap text-2xs gap-2">
          {dec?.action && (
            <Badge variant={actionVariant(dec.action)} size="xs">
              {actionLabel(dec.action)}
            </Badge>
          )}
          {regime && (
            <span className="flex items-center gap-1">
              <span className="text-on-surface-tertiary">市况</span>
              <span className="font-semibold text-info-text">{regime}</span>
            </span>
          )}
          {interval && (
            <span className="flex items-center gap-1">
              <span className="text-on-surface-faint">·</span>
              <span className="text-on-surface-tertiary">间隔</span>
              <span className="font-mono tabular-nums text-on-surface-secondary">{interval}</span>
            </span>
          )}
          {lastDecisionTime && (
            <span className="flex items-center gap-1">
              <span className="text-on-surface-faint">·</span>
              <span className="text-on-surface-tertiary">决策</span>
              <span className="font-mono tabular-nums text-on-surface-secondary">
                {lastDecisionTime}
              </span>
            </span>
          )}
          {runningDuration && (
            <span className="flex items-center gap-1">
              <span className="text-on-surface-faint">·</span>
              <span className="text-on-surface-tertiary">运行</span>
              <span className="font-mono tabular-nums text-on-surface-secondary">
                {runningDuration}
              </span>
            </span>
          )}
        </div>

        {}
        {stats && (
          <div className="flex items-center flex-wrap text-2xs">
            <MetaItem>
              <span className="text-on-surface-tertiary">胜率</span>
              <span className="font-semibold font-mono tabular-nums text-on-surface">
                {stats.win_rate.toFixed(1)}%
              </span>
            </MetaItem>
            <MetaItem>
              <span className="text-on-surface-tertiary">盈亏比</span>
              <span className="font-semibold font-mono tabular-nums text-on-surface">
                {stats.profit_loss_ratio.toFixed(2)}
              </span>
            </MetaItem>
            <MetaItem>
              <span className="text-on-surface-tertiary">回撤</span>
              <span className="font-semibold font-mono tabular-nums text-danger-text">
                {stats.max_drawdown > 0
                  ? formatSigned(-stats.max_drawdown)
                  : formatCompact(stats.max_drawdown)}
              </span>
            </MetaItem>
            <MetaItem>
              <span className="font-mono tabular-nums text-on-surface-tertiary">
                {stats.total_trades}笔
              </span>
            </MetaItem>
          </div>
        )}

        {}
        {stats && (
          <div className="text-2xs text-on-surface-muted font-mono tabular-nums leading-relaxed">
            连胜 <span className="text-success-text">{stats.max_win_streak}</span>
            <span className="text-on-surface-faint mx-1">·</span>
            连亏 <span className="text-danger-text">{stats.max_loss_streak}</span>
            {stats.avg_hold_time && (
              <>
                <span className="text-on-surface-faint mx-1">·</span>
                均持 <span className="text-on-surface-secondary">{stats.avg_hold_time}</span>
              </>
            )}
            <span className="text-on-surface-faint mx-1">·</span>
            手续费{' '}
            <span className="text-on-surface-tertiary">
              {stats.total_fee > 0
                ? formatSigned(-stats.total_fee)
                : formatCompact(stats.total_fee)}
            </span>
            <span className="text-on-surface-faint mx-1">·</span>
            净盈亏{' '}
            <span
              className={stats.net_pnl_after_fee >= 0 ? 'text-success-text' : 'text-danger-text'}
            >
              {formatSigned(stats.net_pnl_after_fee)}
            </span>
          </div>
        )}
      </div>
    );
  },
);

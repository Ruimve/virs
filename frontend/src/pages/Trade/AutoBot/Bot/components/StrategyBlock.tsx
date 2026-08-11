import { memo, useMemo, useState, type ReactNode } from 'react';
import type { AnalysisLog, AutoBot, StrategyDetail } from '@/service/types';
import { Badge } from '@/components/Badge';
import { Sparkline } from '@/components/Sparkline';
import { IndicatorChip, type IndicatorSentiment } from '@/components/IndicatorChip';
import { actionLabel, actionVariant } from '../../../components/utils/utils';
import type { StrategyBlockProps } from './types';
import {
  formatCompact,
  formatSigned,
  formatRelativeTime,
  formatDuration,
  formatInterval,
} from './utils';

const SPARKLINE_PLACEHOLDER = [0.5, 0.6, 0.55, 0.7, 0.65, 0.8, 0.87];

const PLACEHOLDER_INDICATORS: { name: string; value: string; sentiment: IndicatorSentiment }[] = [
  { name: 'RSI', value: '—', sentiment: 'neutral' },
  { name: 'MACD', value: '—', sentiment: 'neutral' },
  { name: 'EMA', value: '—', sentiment: 'neutral' },
  { name: 'BOLL', value: '—', sentiment: 'neutral' },
  { name: 'ATR', value: '—', sentiment: 'neutral' },
  { name: 'ADX', value: '—', sentiment: 'neutral' },
];

function formatCountdown(ms: number): string {
  if (ms <= 0) return '0s';
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const remS = s % 60;
  if (m < 60) return remS > 0 ? `${m}m${remS}s` : `${m}m`;
  const h = Math.floor(m / 60);
  const remM = m % 60;
  return remM > 0 ? `${h}h${remM}m` : `${h}h`;
}

function formatClockTime(ms: number): string {
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${hh}:${mm}`;
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

function extractIndicators(
  decision: AnalysisLog | null,
): { name: string; value: string; sentiment: IndicatorSentiment }[] {
  const raw = decision?.result?.indicators;
  if (Array.isArray(raw) && raw.length > 0) {
    return raw.map((ind) => {
      const s = ind?.sentiment as string | undefined;
      const sentiment: IndicatorSentiment =
        s === 'bullish' || s === 'bearish' || s === 'neutral' ? s : 'neutral';
      return {
        name: String(ind?.name ?? ind?.key ?? '?'),
        value: String(ind?.value ?? ind?.val ?? '-'),
        sentiment,
      };
    });
  }
  return PLACEHOLDER_INDICATORS;
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
    const [expanded, setExpanded] = useState(false);
    const dec = extractDecision(decision);
    const strategyName = resolveStrategyName(bot, strategy);
    const strategyDesc = strategy?.description;
    const direction = deriveDirection(dec?.action);
    const reason = dec?.reason || bot.ai_analysis || '';
    const confidence = dec?.confidence;
    const model = decision?.llm_model;
    const regime = bot.market_regime;

    const indicators = useMemo(() => extractIndicators(decision), [decision]);

    const pnlClass =
      totalPnl > 0 ? 'text-success-text' : totalPnl < 0 ? 'text-danger-text' : 'text-on-surface';

    const interval = formatInterval(decideIntervalSecs);
    const lastDecisionTime = formatRelativeTime(decision?.created_at);
    const runningDuration = formatDuration(bot.created_at);

    const nextDecisionMs =
      decision?.created_at && decideIntervalSecs > 0
        ? new Date(decision.created_at).getTime() + decideIntervalSecs * 1000
        : null;
    const remainingMs = nextDecisionMs != null ? nextDecisionMs - Date.now() : null;

    return (
      <div className="border border-line-default rounded-lg p-2.5 flex flex-col gap-2 bg-surface-1">
        {}
        <div
          onClick={() => setExpanded((e) => !e)}
          className="flex items-center justify-between cursor-pointer select-none"
        >
          <div className="flex items-center gap-1.5">
            <span className="inline-block w-1 h-2.5 bg-ai animate-cursor-blink" />
            <span className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider">
              AI 自主策略
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            {model && <span className="text-2xs text-on-surface-muted font-mono">{model}</span>}
            <Sparkline data={SPARKLINE_PLACEHOLDER} width={40} height={14} />
            {confidence != null && !isNaN(confidence) && (
              <span className="text-2xs font-mono tabular-nums text-ai">
                {Math.round(confidence * 100)}%
              </span>
            )}
            <svg
              className={`w-3 h-3 text-on-surface-muted transition-transform ${expanded ? 'rotate-180' : ''}`}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </div>
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
        <div
          className={`overflow-hidden transition-all duration-300 ease-out ${expanded ? 'max-h-160 opacity-100' : 'max-h-0 opacity-0'}`}
        >
          <div className="flex flex-col gap-2">
            {}
            {(reason || strategyDesc) && (
              <div className="bg-ai-bg rounded-md px-2 py-1 text-2xs text-on-surface-secondary leading-relaxed">
                {reason || strategyDesc}
              </div>
            )}

            {}
            <div className="flex items-center flex-wrap gap-1">
              {indicators.map((ind) => (
                <IndicatorChip
                  key={ind.name}
                  name={ind.name}
                  value={ind.value}
                  sentiment={ind.sentiment}
                />
              ))}
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
                  className={
                    stats.net_pnl_after_fee >= 0 ? 'text-success-text' : 'text-danger-text'
                  }
                >
                  {formatSigned(stats.net_pnl_after_fee)}
                </span>
              </div>
            )}

            {}
            {nextDecisionMs != null && remainingMs != null && (
              <div className="text-2xs text-on-surface-muted font-mono tabular-nums">
                下次决策{' '}
                <span className="text-on-surface-tertiary">{formatClockTime(nextDecisionMs)}</span>
                <span className="text-on-surface-faint mx-1">·</span>
                倒计时 <span className="text-ai">{formatCountdown(remainingMs)}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  },
);

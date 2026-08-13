import { memo, useMemo, useState, type ReactNode } from 'react';
import type { AnalysisLog } from '@/service/types';
import { Radar } from '@/components/Radar';
import { Progress } from '@/components/Progress';
import { IndicatorChip, type IndicatorSentiment } from '../../../components/IndicatorChip';
import { Badge } from '@/components/Badge';
import {
  actionLabel,
  actionVariant,
  executionStatusLabel,
  executionStatusVariant,
} from '../../../components/utils/utils';
import {
  getDecision,
  extractRadarData,
  extractTargetPrices,
  toSentiment,
} from '../../../components/utils/logUtils';
import type { AIPanelProps } from './types';
import { formatCompact, formatRelativeTime } from '../components/utils';

function extractIndicators(
  log: AnalysisLog | null,
): Array<{ name: string; value: string; sentiment: IndicatorSentiment }> {
  const market = log?.result?.market;
  if (!market || typeof market !== 'object') {
    return [
      { name: 'RSI', value: '—', sentiment: 'neutral' },
      { name: 'MACD', value: '—', sentiment: 'neutral' },
      { name: 'EMA', value: '—', sentiment: 'neutral' },
      { name: 'BOLL', value: '—', sentiment: 'neutral' },
      { name: 'ATR', value: '—', sentiment: 'neutral' },
      { name: 'ADX', value: '—', sentiment: 'neutral' },
    ];
  }

  return [
    {
      name: 'RSI',
      value: market.rsi != null ? Number(market.rsi).toFixed(1) : '—',
      sentiment: toSentiment(market.rsi_signal),
    },
    {
      name: 'MACD',
      value: market.macd != null ? Number(market.macd).toFixed(4) : '—',
      sentiment: toSentiment(market.macd_signal),
    },
    {
      name: 'EMA',
      value: market.ema != null ? Number(market.ema).toFixed(2) : '—',
      sentiment: toSentiment(market.ema_signal),
    },
    {
      name: 'BOLL',
      value: market.boll != null ? String(market.boll) : '—',
      sentiment: toSentiment(market.boll_signal),
    },
    {
      name: 'ATR',
      value: market.atr != null ? Number(market.atr).toFixed(2) : '—',
      sentiment: 'neutral',
    },
    {
      name: 'ADX',
      value: market.adx != null ? Number(market.adx).toFixed(1) : '—',
      sentiment: toSentiment(market.adx_signal),
    },
  ];
}

function decisionBadge(action: string | undefined): ReactNode {
  if (!action) return null;
  const label = actionLabel(action);
  if (!label) return null;
  return (
    <Badge variant={actionVariant(action)} size="xs">
      {label}
    </Badge>
  );
}

function formatCountdown(secs: number, lastDecisionTime: string | undefined): string {
  if (secs <= 0) return '';
  const last = lastDecisionTime ? new Date(lastDecisionTime).getTime() : 0;
  if (!last || isNaN(last)) return '';
  const next = last + secs * 1000;
  const remaining = next - Date.now();
  if (remaining <= 0) return '即将决策';
  const s = Math.floor(remaining / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rs = s % 60;
  return `${m}m${rs}s`;
}

export const AIPanel = memo(({ decision, logs, decideIntervalSecs }: AIPanelProps) => {
  const [collapsed, setCollapsed] = useState(false);

  const dec = useMemo(() => getDecision(decision), [decision]);
  const radarData = useMemo(() => extractRadarData(decision), [decision]);
  const indicators = useMemo(() => extractIndicators(decision), [decision]);
  const targets = useMemo(() => extractTargetPrices(decision), [decision]);
  const reason = useMemo(() => dec?.reason || decision?.result?.analysis || '', [dec, decision]);
  const countdown = useMemo(
    () => formatCountdown(decideIntervalSecs, decision?.created_at),
    [decideIntervalSecs, decision?.created_at],
  );

  const recentLogs = useMemo(() => logs.slice(0, 5), [logs]);

  return (
    <div className="rounded-lg border border-ai-border bg-surface-1 overflow-hidden">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center justify-between gap-2 px-3 py-2 bg-ai-bg/30 hover:bg-ai-bg/50 transition-colors cursor-pointer"
      >
        <div className="flex items-center gap-2">
          <span className="inline-block w-1 h-3.5 bg-ai rounded-full animate-cursor-blink" />
          <span className="text-xs font-semibold text-ai tracking-wide">AI 决策引擎</span>
          {countdown && (
            <span className="text-2xs font-mono tabular-nums text-on-surface-tertiary">
              下次 {countdown}
            </span>
          )}
        </div>
        <svg
          className={`w-4 h-4 text-on-surface-muted transition-transform ${collapsed ? '' : 'rotate-180'}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {!collapsed && (
        <div className="flex flex-col gap-3 p-3">
          <div className="flex items-start gap-3">
            <div className="flex items-center justify-center shrink-0">
              <Radar data={radarData} size={120} />
            </div>

            <div className="flex flex-col gap-2 flex-1 min-w-0">
              <div className="flex items-center gap-2">
                {decisionBadge(dec?.action)}
                {decision?.llm_model && (
                  <span className="text-2xs text-on-surface-muted font-mono truncate">
                    {decision.llm_model}
                  </span>
                )}
              </div>

              {dec?.confidence != null && !isNaN(dec.confidence) && (
                <Progress pct={dec.confidence} color="ai" showValue />
              )}

              {(targets.entry || targets.stopLoss || targets.takeProfit) && (
                <div className="grid grid-cols-3 gap-1.5">
                  <div className="flex flex-col gap-0.5">
                    <span className="text-2xs text-on-surface-tertiary">入场</span>
                    <span className="text-xs font-mono font-semibold tabular-nums text-on-surface">
                      {targets.entry ? formatCompact(targets.entry) : '—'}
                    </span>
                  </div>
                  <div className="flex flex-col gap-0.5">
                    <span className="text-2xs text-on-surface-tertiary">止损</span>
                    <span className="text-xs font-mono font-semibold tabular-nums text-danger-text">
                      {targets.stopLoss ? formatCompact(targets.stopLoss) : '—'}
                    </span>
                  </div>
                  <div className="flex flex-col gap-0.5">
                    <span className="text-2xs text-on-surface-tertiary">止盈</span>
                    <span className="text-xs font-mono font-semibold tabular-nums text-success-text">
                      {targets.takeProfit ? formatCompact(targets.takeProfit) : '—'}
                    </span>
                  </div>
                </div>
              )}
            </div>
          </div>

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

          {reason && (
            <div className="bg-ai-bg rounded-md p-2 flex flex-col gap-1">
              <span className="text-2xs font-semibold text-ai uppercase tracking-wider">
                AI 推理
              </span>
              <p className="text-2xs text-on-surface-secondary leading-relaxed line-clamp-4">
                {reason}
              </p>
            </div>
          )}

          {recentLogs.length > 0 && (
            <div className="flex flex-col gap-1">
              <span className="text-2xs font-semibold text-on-surface-tertiary uppercase tracking-wider">
                决策历史
              </span>
              <div className="flex flex-col gap-0.5">
                {recentLogs.map((log) => {
                  const d = getDecision(log);
                  return (
                    <div key={log.id} className="flex items-center gap-2 text-2xs py-0.5">
                      <span className="text-on-surface-muted font-mono tabular-nums shrink-0 w-12">
                        {formatRelativeTime(log.created_at)}
                      </span>
                      {d?.action && (
                        <Badge variant={actionVariant(d.action)} size="xs">
                          {actionLabel(d.action)}
                        </Badge>
                      )}
                      {log.execution_status && (
                        <Badge variant={executionStatusVariant(log.execution_status)} size="xs">
                          {executionStatusLabel(log.execution_status)}
                        </Badge>
                      )}
                      {d?.confidence != null && !isNaN(d.confidence) && (
                        <span className="font-mono tabular-nums text-on-surface-tertiary shrink-0">
                          {(d.confidence * 100).toFixed(0)}%
                        </span>
                      )}
                      <span className="text-on-surface-muted truncate">
                        {d?.reason || log.result?.analysis || '—'}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
});

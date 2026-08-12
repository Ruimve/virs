import { useState, useEffect, useCallback, memo } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import type { FlowStepStatus } from '@/components/FlowStep';
import { Check, Spinner, Warning } from '@/components/Icon';
import { Button } from '@/components/Button';
import {
  getAiStatus,
  testAiCredential,
  fetchCredentialStatus,
  testCredential,
  checkHealth,
  getPaperStatus,
} from '@/service';
import { getSystemInfo } from '@/service/system';
import type { SystemInfo } from '@/service/types';
import { useBot } from '../context/BotContext';
import type { CheckData, CheckItem, CheckKey } from './define';
import { useShell } from '@/context/ShellContext';

const INITIAL_CHECKS: CheckItem[] = [
  {
    key: 'llm',
    label: 'LLM Connection',
    description: 'AI analysis provider reachability',
    status: 'pending',
    detail: '',
  },
  {
    key: 'exchange',
    label: 'Exchange API',
    description: 'Exchange API credentials & latency',
    status: 'pending',
    detail: '',
  },
  {
    key: 'api',
    label: 'API Server',
    description: 'Backend service availability',
    status: 'pending',
    detail: '',
  },
  {
    key: 'system',
    label: 'System Resources',
    description: 'CPU, memory & host health',
    status: 'pending',
    detail: '',
  },
  {
    key: 'trading',
    label: 'Trading Engine',
    description: 'Paper vs. live execution mode',
    status: 'pending',
    detail: '',
  },
];

/* ── Step card visual configs per status ── */

interface StepVisual {
  border: string;
  bg: string;
  iconBg: string;
  iconContent: React.ReactNode;
  label: string;
  labelBg: string;
  labelText: string;
  titleClass: string;
  detailClass: string;
}

const StepCard = memo(({ item }: { item: CheckItem }) => {
  const visuals: Record<FlowStepStatus, StepVisual> = {
    done: {
      border: 'border-success-border',
      bg: 'bg-success-bg',
      iconBg: 'bg-success',
      iconContent: <Check className="w-3.5 h-3.5 text-white" strokeWidth={3} />,
      labelBg: 'bg-success/15',
      labelText: 'text-success-text',
      label: 'Passed',
      titleClass: 'text-on-base',
      detailClass: 'text-on-surface-tertiary',
    },
    verifying: {
      border: 'border-info-border',
      bg: 'bg-info-bg',
      iconBg: 'bg-info shadow-glow-info',
      iconContent: <Spinner className="w-3.5 h-3.5 text-white" />,
      labelBg: 'bg-info/15',
      labelText: 'text-info-text',
      label: 'Running',
      titleClass: 'text-on-base',
      detailClass: 'text-info-text',
    },
    error: {
      border: 'border-danger-border',
      bg: 'bg-danger-bg',
      iconBg: 'bg-danger',
      iconContent: <Warning className="w-3.5 h-3.5 text-white" strokeWidth={2.5} />,
      labelBg: 'bg-danger/15',
      labelText: 'text-danger-text',
      label: 'Failed',
      titleClass: 'text-on-base',
      detailClass: 'text-danger-text',
    },
    active: {
      border: 'border-accent-muted',
      bg: 'bg-accent-light',
      iconBg: 'bg-accent',
      iconContent: <Spinner className="w-3.5 h-3.5 text-white" />,
      labelBg: 'bg-accent/15',
      labelText: 'text-accent',
      label: 'Active',
      titleClass: 'text-on-base',
      detailClass: 'text-on-surface-tertiary',
    },
    pending: {
      border: 'border-line-default',
      bg: 'bg-surface-1',
      iconBg: 'border-2 border-dashed border-on-surface-muted',
      iconContent: <span className="text-on-surface-muted text-xs">○</span>,
      labelBg: 'bg-surface-3',
      labelText: 'text-on-surface-muted',
      label: 'Pending',
      titleClass: 'text-on-surface-muted',
      detailClass: 'text-on-surface-muted',
    },
  };

  const v = visuals[item.status];

  return (
    <div
      className={`flex items-center gap-3 px-4 py-3.5 rounded-xl border ${v.border} ${v.bg} transition-all duration-300`}
    >
      {/* Status icon */}
      <div
        className={`shrink-0 w-6.5 h-6.5 rounded-full flex items-center justify-center ${v.iconBg}`}
      >
        {v.iconContent}
      </div>

      {/* Title + detail */}
      <div className="flex-1 min-w-0">
        <div className={`text-sm font-semibold ${v.titleClass}`}>{item.label}</div>
        <div className={`text-2xs font-mono mt-0.5 ${v.detailClass} truncate`}>
          {item.status === 'verifying'
            ? 'Checking...'
            : item.status === 'pending'
              ? 'Waiting...'
              : item.data?.message && item.status === 'error'
                ? item.data.message
                : item.detail || item.description}
        </div>
      </div>

      {/* Status label */}
      <span
        className={`shrink-0 text-2xs font-semibold px-2 py-0.5 rounded-md ${v.labelBg} ${v.labelText} ${item.status === 'verifying' ? 'animate-pulse' : ''}`}
      >
        {v.label}
      </span>
    </div>
  );
});

StepCard.displayName = 'StepCard';

/* ── Error retry row ── */

const ErrorRetry = memo(({ item, onRetry }: { item: CheckItem; onRetry: () => void }) => {
  if (item.status !== 'error') return null;
  return (
    <div className="flex items-center gap-2 pl-10 pr-4 pb-2 -mt-1">
      <Button variant="ghost" size="xs" responsive={false} onClick={onRetry}>
        Retry
      </Button>
    </div>
  );
});

ErrorRetry.displayName = 'ErrorRetry';

/* ── Main component ── */

const HealthCheck = () => {
  const navigate = useNavigate();
  const param = useParams();
  const { setNavItems } = useShell();
  const { bot } = useBot();

  const [checks, setChecks] = useState<CheckItem[]>(INITIAL_CHECKS);
  const [running, setRunning] = useState(false);

  const updateCheck = useCallback(
    (key: CheckKey, status: FlowStepStatus, detail: string, data?: CheckData) => {
      setChecks((prev) => prev.map((c) => (c.key === key ? { ...c, status, detail, data } : c)));
    },
    [],
  );

  const runLlmCheck = useCallback(async () => {
    updateCheck('llm', 'verifying', '');
    try {
      const status = await getAiStatus();
      if (!status.success || !status.data?.configured) {
        updateCheck('llm', 'error', 'Not configured', {
          message: 'Add an AI provider in Setup to enable AI-driven analysis.',
        });
        return;
      }
      const providers = status.data.providers ?? [];
      const test = await testAiCredential();
      if (test.success && test.data?.connected) {
        updateCheck('llm', 'done', `Reachable · ${providers.join(', ') || 'ok'}`, {
          providers,
        });
      } else {
        updateCheck('llm', 'error', 'Unreachable', {
          providers,
          message: test.message || test.data?.message || 'Connection test failed.',
        });
      }
    } catch {
      updateCheck('llm', 'error', 'Connection failed');
    }
  }, [updateCheck]);

  const runExchangeCheck = useCallback(async () => {
    updateCheck('exchange', 'verifying', '');
    try {
      const status = await fetchCredentialStatus();
      if (!status.success || !status.data?.connected) {
        updateCheck('exchange', 'error', 'Not connected', {
          message: 'Save exchange credentials in Setup to enable trading.',
        });
        return;
      }
      const exchange = status.data.exchange ?? 'exchange';
      const test = await testCredential();
      if (test.success && test.data?.connected) {
        updateCheck('exchange', 'done', `${exchange} · Connected`, { exchange });
      } else {
        updateCheck('exchange', 'error', 'Ping failed', {
          exchange,
          message: test.data?.message || test.message || 'Exchange did not respond.',
        });
      }
    } catch {
      updateCheck('exchange', 'error', 'Connection failed');
    }
  }, [updateCheck]);

  const runApiCheck = useCallback(async () => {
    updateCheck('api', 'verifying', '');
    try {
      const res = await checkHealth();
      if (res.success && res.data?.status === 'ok') {
        const version = res.data.version;
        updateCheck('api', 'done', version ? `Online · v${version}` : 'Online', { version });
      } else {
        updateCheck('api', 'error', 'Unavailable', {
          message: res.message || 'Backend service is not responding.',
        });
      }
    } catch {
      updateCheck('api', 'error', 'Connection failed');
    }
  }, [updateCheck]);

  const runSystemCheck = useCallback(async () => {
    updateCheck('system', 'verifying', '');
    try {
      const res = await getSystemInfo();
      if (!res.success || !res.data) {
        updateCheck('system', 'error', 'Unable to read metrics', {
          message: res.message || 'System metrics unavailable.',
        });
        return;
      }
      const info: SystemInfo = res.data;
      const cpuPct = info.cpu.usage_pct;
      const memPct = info.memory.usage_pct;
      const critical = cpuPct >= 90 || memPct >= 90;
      const detail = `CPU ${cpuPct.toFixed(0)}% · Mem ${memPct.toFixed(0)}%`;
      updateCheck(
        'system',
        critical ? 'error' : 'done',
        critical ? `Critical load · ${detail}` : detail,
        { cpuPct, memPct, coreCount: info.cpu.core_count, host: info.host_name },
      );
    } catch {
      updateCheck('system', 'error', 'Connection failed');
    }
  }, [updateCheck]);

  const runTradingCheck = useCallback(async () => {
    updateCheck('trading', 'verifying', '');
    try {
      const res = await getPaperStatus();
      if (!res.success || res.data === undefined) {
        updateCheck('trading', 'error', 'Unknown', {
          message: res.message || 'Unable to determine trading mode.',
        });
        return;
      }
      const paperMode = res.data.paper_mode;
      const restoreError = res.data.restore_error;
      if (restoreError) {
        updateCheck('trading', 'error', 'Restore failed', {
          message: `Service restore failed: ${restoreError}. All bots have been marked as error. Fix the issue and restart.`,
        });
        return;
      }
      if (paperMode === null) {
        updateCheck('trading', 'error', 'Engines not started', {
          message: 'Trading engines have not been started. Create or start a bot to initialize.',
        });
        return;
      }
      updateCheck('trading', 'done', paperMode ? 'Paper trading' : 'Live trading', {
        paperMode,
      });
    } catch {
      updateCheck('trading', 'error', 'Connection failed');
    }
  }, [updateCheck]);

  const checkRunners: Record<CheckKey, () => Promise<void>> = {
    llm: runLlmCheck,
    exchange: runExchangeCheck,
    api: runApiCheck,
    system: runSystemCheck,
    trading: runTradingCheck,
  };

  const runChecks = useCallback(async () => {
    setRunning(true);
    await Promise.allSettled([
      runLlmCheck(),
      runExchangeCheck(),
      runApiCheck(),
      runSystemCheck(),
      runTradingCheck(),
    ]);
    setRunning(false);
  }, [runLlmCheck, runExchangeCheck, runApiCheck, runSystemCheck, runTradingCheck]);

  useEffect(() => {
    setNavItems([]);
  }, [setNavItems]);

  useEffect(() => {
    runChecks();
  }, [runChecks]);

  const total = checks.length;
  const passed = checks.filter((c) => c.status === 'done').length;
  const failed = checks.filter((c) => c.status === 'error').length;
  const settled = passed + failed;
  const allDone = settled === total;

  const handleContinue = () => {
    navigate(`/trade/${param.botType}/${bot?.id}`, { replace: true });
  };

  const progressPct = total > 0 ? (settled / total) * 100 : 0;

  // Status badge config
  const badgeConfig = running
    ? {
        bg: 'bg-accent-light',
        text: 'text-accent',
        border: 'border-accent-muted',
        label: 'Running',
        dot: 'bg-accent',
        pulse: true,
      }
    : failed === 0
      ? {
          bg: 'bg-success-bg',
          text: 'text-success-text',
          border: 'border-success-border',
          label: 'Operational',
          dot: 'bg-success',
          pulse: false,
        }
      : {
          bg: 'bg-warning-bg',
          text: 'text-warning-text',
          border: 'border-warning-border',
          label: 'Issues',
          dot: 'bg-warning',
          pulse: false,
        };

  const progressBarColor =
    running || settled < total ? 'bg-accent' : failed > 0 ? 'bg-warning' : 'bg-success';

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-lg mx-auto px-4 md:px-8 pt-8 md:pt-12 pb-6">
        {/* Title */}
        <div className="text-center mb-6">
          <h2 className="text-xl md:text-2xl font-extralight tracking-wide text-on-base">
            Health Check
          </h2>
          <p className="mt-1.5 text-sm text-on-surface-tertiary">Pre-launch verification</p>
        </div>

        {/* Status summary card */}
        <div className="mb-5 rounded-xl border border-line-subtle bg-surface-1 p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2.5">
              <span
                className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-caption font-medium border ${badgeConfig.bg} ${badgeConfig.text} ${badgeConfig.border}`}
              >
                {running && <Spinner className="w-3 h-3" />}
                {!running && (
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${badgeConfig.dot} ${badgeConfig.pulse ? 'animate-pulse' : ''}`}
                  />
                )}
                {badgeConfig.label}
              </span>
              <span className="text-xs font-mono text-on-surface-secondary">
                {passed} of {total} passed
              </span>
            </div>
            <Button
              variant="accent-outline"
              size="small"
              responsive={false}
              onClick={runChecks}
              loading={running}
            >
              Re-run
            </Button>
          </div>
          {/* Progress bar */}
          <div className="flex items-center gap-2.5">
            <div className="flex-1 h-1.5 rounded-full bg-surface-3 overflow-hidden">
              <div
                className={`h-full ${progressBarColor} rounded-full transition-all duration-500`}
                style={{ width: `${progressPct}%` }}
              />
            </div>
            <span className="text-xs font-mono font-semibold tabular-nums text-on-surface-secondary">
              {progressPct.toFixed(0)}%
            </span>
          </div>
        </div>

        {/* Check steps */}
        <div className="flex flex-col gap-2.5">
          {checks.map((check) => (
            <div key={check.key}>
              <StepCard item={check} />
              <ErrorRetry item={check} onRetry={() => checkRunners[check.key]()} />
            </div>
          ))}
        </div>

        {/* Continue button — only when all checks settled */}
        {allDone && (
          <div className="mt-6 flex items-center justify-between">
            <p className="text-xs text-on-surface-tertiary">
              {failed > 0
                ? `${failed} check${failed > 1 ? 's' : ''} failed — you can still continue.`
                : 'All checks passed.'}
            </p>
            <Button variant="primary" size="normal" responsive={false} onClick={handleContinue}>
              Continue
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};

export default HealthCheck;

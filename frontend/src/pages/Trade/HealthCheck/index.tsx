import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { FlowSteps, type FlowStepConfig, type FlowStepStatus } from '@/components/FlowStep';
import { Spinner, ChevronRight } from '@/components/Icon';
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
import { useHeader } from '../Layout/Header/HeaderContext';
import type { CheckData, CheckItem, CheckKey } from './define';
import CheckDetail from './CheckDetail';

// ── 检查项初始配置 ──────────────────────────────────────────
const INITIAL_CHECKS: CheckItem[] = [
  {
    key: 'llm',
    label: 'LLM Connectivity',
    description: 'AI analysis provider reachability',
    status: 'pending',
    detail: '',
  },
  {
    key: 'exchange',
    label: 'Exchange Connectivity',
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
    label: 'Trading Mode',
    description: 'Paper vs. live execution mode',
    status: 'pending',
    detail: '',
  },
];

const HealthCheck = () => {
  const navigate = useNavigate();
  const param = useParams();
  const { updateTabs } = useHeader();
  const { bot } = useBot();

  const [checks, setChecks] = useState<CheckItem[]>(INITIAL_CHECKS);
  const [running, setRunning] = useState(false);

  const updateCheck = useCallback(
    (key: CheckKey, status: FlowStepStatus, detail: string, data?: CheckData) => {
      setChecks((prev) => prev.map((c) => (c.key === key ? { ...c, status, detail, data } : c)));
    },
    [],
  );

  // ── 单项检查逻辑（每项独立可重试）──────────────────────────

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
          message: test.error || test.data?.message || 'Connection test failed.',
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
          message: test.data?.message || test.error || 'Exchange did not respond.',
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
          message: res.error || 'Backend service is not responding.',
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
          message: res.error || 'System metrics unavailable.',
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
          message: res.error || 'Unable to determine trading mode.',
        });
        return;
      }
      const paperMode = res.data.paper_mode;
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

  // 并行执行所有检查（替代原有的串行 + setTimeout 模拟）
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
    updateTabs([{ key: 'health', label: 'Health Check', onClick: () => {} }]);
  }, [updateTabs]);

  useEffect(() => {
    runChecks();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── 派生状态 ──────────────────────────────────────────────

  const total = checks.length;
  const passed = checks.filter((c) => c.status === 'done').length;
  const failed = checks.filter((c) => c.status === 'error').length;
  const settled = passed + failed;
  const allDone = settled === total;

  const statuses: Record<string, FlowStepStatus> = {};
  for (const c of checks) statuses[c.key] = c.status;

  const summaries: Record<string, string> = {};
  for (const c of checks) {
    if (c.status === 'done' && c.detail) summaries[c.key] = c.detail;
  }

  const steps: FlowStepConfig[] = checks.map((c) => ({
    key: c.key,
    title: c.label,
    description: c.description,
    render: () => <CheckDetail item={c} onRetry={() => checkRunners[c.key]()} />,
  }));

  const handleContinue = () => {
    navigate(`/trade/${param.botType}/${bot?.id}`, { replace: true });
  };

  // ── 概览状态 ──────────────────────────────────────────────

  const tone = running ? 'accent' : failed === 0 ? 'success' : 'warning';
  const toneClasses: Record<string, string> = {
    accent: 'bg-accent-light text-accent',
    success: 'bg-success-bg text-success-text',
    warning: 'bg-warning-bg text-warning-text',
  };
  const headline = running
    ? 'Checking system…'
    : failed === 0
      ? 'All systems operational'
      : `${failed} of ${total} checks failed`;
  const progressPct = total > 0 ? (settled / total) * 100 : 0;
  const progressBar =
    running || settled < total ? 'bg-accent' : failed > 0 ? 'bg-warning' : 'bg-success';

  return (
    <div className="max-w-lg mx-auto px-4 md:px-8 pt-8 md:pt-12 pb-6">
      {/* 标题 */}
      <div className="mb-6">
        <h2 className="text-xl md:text-2xl font-extralight tracking-wide text-on-base">
          Health Check
        </h2>
        <p className="mt-1.5 text-sm text-on-surface-tertiary">
          Pre-flight verification of system components
        </p>
      </div>

      {/* 概览卡片 */}
      <div className="mb-6 rounded-xl border border-line-subtle bg-surface-1 p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span
              className={`inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px] font-medium ${toneClasses[tone]}`}
            >
              {running && <Spinner className="w-3 h-3" />}
              {running ? 'Running' : failed === 0 ? 'Operational' : 'Issues'}
            </span>
            <div>
              <p className="text-sm font-medium text-on-surface">{headline}</p>
              <p className="text-[12px] text-on-surface-tertiary mt-0.5">
                {passed} of {total} checks passed
              </p>
            </div>
          </div>
          <button
            onClick={runChecks}
            disabled={running}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-line-default text-on-surface-secondary text-[12px] font-medium hover:bg-surface-2 disabled:opacity-40 transition-colors"
          >
            {running && <Spinner className="w-3.5 h-3.5" />}
            {running ? 'Running' : 'Re-run'}
          </button>
        </div>
        <div className="mt-3 h-1 w-full bg-surface-2 rounded-full overflow-hidden">
          <div
            className={`h-full ${progressBar} rounded-full transition-all duration-500`}
            style={{ width: `${progressPct}%` }}
          />
        </div>
      </div>

      {/* 检查步骤 */}
      <FlowSteps steps={steps} statuses={statuses} summaries={summaries} />

      {/* 继续操作 */}
      {allDone && (
        <div className="mt-6 flex items-center justify-between">
          <p className="text-[12px] text-on-surface-tertiary">
            {failed > 0
              ? `${failed} check${failed > 1 ? 's' : ''} failed — you can still continue.`
              : 'All checks passed.'}
          </p>
          <button
            onClick={handleContinue}
            className="inline-flex items-center gap-1 px-6 py-2.5 bg-accent/80 hover:bg-accent-hover text-white text-sm font-medium rounded-xl transition-all duration-200"
          >
            Continue
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      )}
    </div>
  );
};

export default HealthCheck;

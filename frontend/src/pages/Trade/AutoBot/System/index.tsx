import { useEffect, useState, useCallback, useRef } from 'react';
import { getSystemInfo } from '@/service/system';
import type { SystemInfo as SystemInfoData } from '@/service/types';
import { Card } from '@/components/Card';
import { Progress } from '@/components/Progress';
import { usageColor } from '@/components/Progress/utils';
import { StateFeedback } from '@/components/StateFeedback';

// -- Utility functions --

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / Math.pow(1024, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatRate(bytesPerSec: number): string {
  return formatBytes(bytesPerSec) + '/s';
}

function formatUptime(secs: number): string {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  const parts: string[] = [];
  if (days > 0) parts.push(`${days}天`);
  if (hours > 0) parts.push(`${hours}小时`);
  parts.push(`${mins}分钟`);
  return parts.join(' ');
}

// -- Sub-components --

interface MiniSparkProps {
  active: boolean;
  direction: 'up' | 'down' | 'flat';
}

const MiniSpark = ({ active, direction }: MiniSparkProps) => {
  if (!active) return <div className="w-8 h-4 bg-surface-2/50 rounded-sm" />;
  const colorClass =
    direction === 'up'
      ? 'text-success'
      : direction === 'down'
        ? 'text-info'
        : 'text-on-surface-muted';
  return (
    <svg className={`w-8 h-4 ${colorClass}`} viewBox="0 0 32 16" fill="none">
      <polyline
        points={
          direction === 'up'
            ? '0,14 6,10 12,12 18,6 24,8 32,2'
            : direction === 'down'
              ? '0,2 6,6 12,4 18,10 24,8 32,14'
              : '0,8 8,8 16,8 24,8 32,8'
        }
        strokeWidth="1.5"
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
        opacity="0.6"
      />
    </svg>
  );
};

// -- Resource icon components --

const CpuIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.5">
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <path d="M9 1v3M15 1v3M9 20v3M15 20v3M1 9h3M1 15h3M20 9h3M20 15h3" strokeLinecap="round" />
  </svg>
);

const MemIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.5">
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <line x1="3" y1="10" x2="21" y2="10" />
    <line x1="8" y1="2" x2="8" y2="6" />
    <line x1="16" y1="2" x2="16" y2="6" />
  </svg>
);

const DiskIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.5">
    <rect x="2" y="6" width="20" height="12" rx="2" />
    <circle cx="17" cy="12" r="1.5" />
    <line x1="6" y1="12" x2="12" y2="12" />
  </svg>
);

const NetIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.5">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 2v4M12 18v4M2 12h4M18 12h4" strokeLinecap="round" />
    <path
      d="M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"
      strokeLinecap="round"
    />
  </svg>
);

const LoadIcon = () => (
  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.5">
    <path d="M12 2v4M12 18v4M2 12h4M18 12h4" strokeLinecap="round" />
    <circle cx="12" cy="12" r="4" />
  </svg>
);

// -- Main component --

interface NetSample {
  total_rx: number;
  total_tx: number;
  ts: number;
}

const System = () => {
  const [info, setInfo] = useState<SystemInfoData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [netRates, setNetRates] = useState<Record<string, { rx: number; tx: number }>>({});
  const lastNetSample = useRef<Record<string, NetSample>>({});

  const loadInfo = useCallback(async () => {
    try {
      const res = await getSystemInfo();
      if (res.success && res.data) {
        setInfo(res.data);
        setError(null);

        const now = Date.now();
        const newRates: Record<string, { rx: number; tx: number }> = {};
        const newSample: Record<string, NetSample> = {};
        for (const net of res.data.network) {
          const prev = lastNetSample.current[net.name];
          if (prev) {
            const dt = (now - prev.ts) / 1000;
            if (dt > 0) {
              newRates[net.name] = {
                rx: Math.max(0, (net.total_rx_bytes - prev.total_rx) / dt),
                tx: Math.max(0, (net.total_tx_bytes - prev.total_tx) / dt),
              };
            }
          }
          newSample[net.name] = {
            total_rx: net.total_rx_bytes,
            total_tx: net.total_tx_bytes,
            ts: now,
          };
        }
        lastNetSample.current = newSample;
        setNetRates(newRates);
      } else {
        setError(res.error || '获取系统信息失败');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '网络错误');
    }
  }, []);

  useEffect(() => {
    loadInfo();
    const id = setInterval(loadInfo, 5000);
    return () => clearInterval(id);
  }, [loadInfo]);

  if (error) {
    return <StateFeedback type="error" text={error} />;
  }

  if (!info) {
    return <StateFeedback type="center-loading" />;
  }

  const cpuPct = info.cpu.usage_pct;
  const memPct = info.memory.usage_pct;

  return (
    <div className="h-full overflow-y-auto p-4 space-y-4">
      {/* System Overview */}
      <Card title="系统概览" icon={<CpuIcon />}>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
          <div>
            <div className="text-[11px] text-on-surface-tertiary mb-1">主机名</div>
            <div className="text-on-surface font-mono tabular-nums truncate">
              {info.host_name || '-'}
            </div>
          </div>
          <div>
            <div className="text-[11px] text-on-surface-tertiary mb-1">操作系统</div>
            <div className="text-on-surface">
              {info.os_name} {info.os_version}
            </div>
          </div>
          <div>
            <div className="text-[11px] text-on-surface-tertiary mb-1">运行时长</div>
            <div className="text-on-surface font-mono tabular-nums">
              {formatUptime(info.uptime_secs)}
            </div>
          </div>
          <div>
            <div className="text-[11px] text-on-surface-tertiary mb-1">进程数</div>
            <div className="text-on-surface font-mono tabular-nums">{info.process_count}</div>
          </div>
        </div>
      </Card>

      {/* CPU & Memory */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card title="CPU" icon={<CpuIcon />}>
          <div className="space-y-3">
            <div>
              <div className="flex items-baseline justify-between mb-1.5">
                <span className="text-[11px] text-on-surface-tertiary">使用率</span>
                <span
                  className={`text-lg font-mono font-semibold tabular-nums ${usageColor(cpuPct)}`}
                >
                  {cpuPct.toFixed(1)}%
                </span>
              </div>
              <Progress pct={cpuPct} />
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-[11px] text-on-surface-tertiary">核心数</span>
              <span className="text-on-surface font-mono tabular-nums">{info.cpu.core_count}</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-[11px] text-on-surface-tertiary">主频</span>
              <span className="text-on-surface font-mono tabular-nums">
                {info.cpu.frequency_mhz > 0
                  ? `${(info.cpu.frequency_mhz / 1000).toFixed(2)} GHz`
                  : '-'}
              </span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-[11px] text-on-surface-tertiary">型号</span>
              <span className="text-on-surface text-[10px] truncate ml-2 max-w-[180px]">
                {info.cpu.brand}
              </span>
            </div>
          </div>
        </Card>

        <Card title="内存" icon={<MemIcon />}>
          <div className="space-y-3">
            <div>
              <div className="flex items-baseline justify-between mb-1.5">
                <span className="text-[11px] text-on-surface-tertiary">使用率</span>
                <span
                  className={`text-lg font-mono font-semibold tabular-nums ${usageColor(memPct)}`}
                >
                  {memPct.toFixed(1)}%
                </span>
              </div>
              <Progress pct={memPct} />
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-[11px] text-on-surface-tertiary">已用 / 总量</span>
              <span className="text-on-surface font-mono tabular-nums">
                {formatBytes(info.memory.used_bytes)} / {formatBytes(info.memory.total_bytes)}
              </span>
            </div>
            {info.swap.total_bytes > 0 && (
              <div className="flex justify-between text-xs">
                <span className="text-[11px] text-on-surface-tertiary">Swap</span>
                <span className="text-on-surface font-mono tabular-nums">
                  {formatBytes(info.swap.used_bytes)} / {formatBytes(info.swap.total_bytes)}
                </span>
              </div>
            )}
          </div>
        </Card>
      </div>

      {/* System Load */}
      <Card title="系统负载（1 / 5 / 15 分钟）" icon={<LoadIcon />}>
        <div className="grid grid-cols-3 gap-4">
          {[
            { label: '1 分钟', value: info.load_average.one },
            { label: '5 分钟', value: info.load_average.five },
            { label: '15 分钟', value: info.load_average.fifteen },
          ].map(({ label, value }) => {
            const loadPct = info.cpu.core_count > 0 ? (value / info.cpu.core_count) * 100 : 0;
            return (
              <div key={label}>
                <div className="flex items-baseline justify-between mb-1.5">
                  <span className="text-[11px] text-on-surface-tertiary">{label}</span>
                  <span
                    className={`text-lg font-mono font-semibold tabular-nums ${usageColor(loadPct)}`}
                  >
                    {value.toFixed(2)}
                  </span>
                </div>
                <Progress pct={Math.min(loadPct, 100)} />
                <div className="text-[10px] text-on-surface-faint mt-1 text-right font-mono tabular-nums">
                  {loadPct.toFixed(0)}% 核心
                </div>
              </div>
            );
          })}
        </div>
      </Card>

      {/* Disk */}
      <Card title="磁盘" icon={<DiskIcon />}>
        <div className="space-y-3">
          {info.disks.map((disk, i) => (
            <div key={i}>
              <div className="flex items-baseline justify-between mb-1.5">
                <span className="text-xs text-on-surface font-mono tabular-nums">
                  {disk.mount_point}
                </span>
                <span className={`text-xs font-mono tabular-nums ${usageColor(disk.usage_pct)}`}>
                  {disk.usage_pct.toFixed(1)}%
                </span>
              </div>
              <Progress pct={disk.usage_pct} />
              <div className="text-[10px] text-on-surface-faint mt-1 font-mono tabular-nums">
                {formatBytes(disk.used_bytes)} / {formatBytes(disk.total_bytes)}
              </div>
            </div>
          ))}
        </div>
      </Card>

      {/* Network */}
      <Card title="网络" icon={<NetIcon />}>
        <div className="space-y-2">
          {info.network.length === 0 ? (
            <div className="text-xs text-on-surface-tertiary">无网络接口</div>
          ) : (
            info.network.map((net, i) => {
              const rate = netRates[net.name];
              const hasRxRate = rate && rate.rx > 0;
              const hasTxRate = rate && rate.tx > 0;
              const direction =
                hasRxRate && !hasTxRate ? 'down' : !hasRxRate && hasTxRate ? 'up' : 'flat';
              return (
                <div
                  key={i}
                  className="py-2 px-3 rounded-lg bg-surface-2/30 hover:bg-surface-2/50 transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <span className="text-on-surface font-mono tabular-nums text-xs shrink-0 w-16">
                      {net.name}
                    </span>
                    <div className="flex-1 flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <span className="text-on-surface-muted text-[11px]">
                          ↓{' '}
                          <span className="text-success-text font-mono tabular-nums">
                            {rate ? formatRate(rate.rx) : '-'}
                          </span>
                        </span>
                        <span className="text-on-surface-muted text-[11px]">
                          ↑{' '}
                          <span className="text-info-text font-mono tabular-nums">
                            {rate ? formatRate(rate.tx) : '-'}
                          </span>
                        </span>
                      </div>
                      <MiniSpark active={!!rate} direction={direction} />
                    </div>
                  </div>
                  {net.ips.length > 0 && (
                    <div className="text-[10px] text-on-surface-faint mt-1 font-mono tabular-nums truncate pl-16">
                      {net.ips.join(', ')}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </Card>
    </div>
  );
};

export default System;

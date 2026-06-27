import { useEffect, useState, useCallback, useRef } from 'react';
import { getSystemInfo } from '@/service/system';
import type { SystemInfo as SystemInfoData } from '@/service/types';
import { Spinner } from '@/components/Icon';

// ── 工具函数 ────────────────────────────────────────────────

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

function usageColor(pct: number): string {
  if (pct >= 90) return 'text-danger-text';
  if (pct >= 70) return 'text-warning-text';
  return 'text-success-text';
}

function barColor(pct: number): string {
  if (pct >= 90) return 'bg-danger';
  if (pct >= 70) return 'bg-warning';
  return 'bg-success';
}

interface ProgressBarProps {
  pct: number;
}

const ProgressBar = ({ pct }: ProgressBarProps) => (
  <div className="h-1.5 bg-surface-2 rounded-full overflow-hidden">
    <div
      className={`h-full ${barColor(pct)} rounded-full transition-all duration-500`}
      style={{ width: `${Math.min(pct, 100)}%` }}
    />
  </div>
);

// ── 卡片组件 ────────────────────────────────────────────────

interface CardProps {
  title: string;
  children: React.ReactNode;
}

const Card = ({ title, children }: CardProps) => (
  <div className="bg-surface-1 border border-line-subtle rounded-xl p-4">
    <h3 className="text-xs font-medium text-on-surface-tertiary mb-3">{title}</h3>
    {children}
  </div>
);

// ── 主组件 ──────────────────────────────────────────────────

// 保存上次网络采样，用于计算速率
interface NetSample {
  total_rx: number;
  total_tx: number;
  ts: number;
}

const System = () => {
  const [info, setInfo] = useState<SystemInfoData | null>(null);
  const [error, setError] = useState<string | null>(null);
  // 网络速率：name → bytes/s
  const [netRates, setNetRates] = useState<Record<string, { rx: number; tx: number }>>({});
  const lastNetSample = useRef<Record<string, NetSample>>({});

  const loadInfo = useCallback(async () => {
    try {
      const res = await getSystemInfo();
      if (res.success && res.data) {
        setInfo(res.data);
        setError(null);

        // 计算网络速率
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
    return (
      <div className="flex-1 flex items-center justify-center text-danger-text text-sm">
        {error}
      </div>
    );
  }

  if (!info) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Spinner className="h-6 w-6 text-on-surface-tertiary" />
      </div>
    );
  }

  const cpuPct = info.cpu.usage_pct;
  const memPct = info.memory.usage_pct;

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-4">
      {/* 系统概览 */}
      <Card title="系统概览">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
          <div>
            <div className="text-on-surface-tertiary mb-0.5">主机名</div>
            <div className="text-on-surface font-mono truncate">{info.host_name || '-'}</div>
          </div>
          <div>
            <div className="text-on-surface-tertiary mb-0.5">操作系统</div>
            <div className="text-on-surface">
              {info.os_name} {info.os_version}
            </div>
          </div>
          <div>
            <div className="text-on-surface-tertiary mb-0.5">运行时长</div>
            <div className="text-on-surface">{formatUptime(info.uptime_secs)}</div>
          </div>
          <div>
            <div className="text-on-surface-tertiary mb-0.5">进程数</div>
            <div className="text-on-surface font-mono">{info.process_count}</div>
          </div>
        </div>
      </Card>

      {/* CPU & 内存 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <Card title="CPU">
          <div className="space-y-3">
            <div>
              <div className="flex items-baseline justify-between mb-1">
                <span className="text-xs text-on-surface-tertiary">使用率</span>
                <span className={`text-sm font-mono font-medium ${usageColor(cpuPct)}`}>
                  {cpuPct.toFixed(1)}%
                </span>
              </div>
              <ProgressBar pct={cpuPct} />
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">核心数</span>
              <span className="text-on-surface font-mono">{info.cpu.core_count}</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">主频</span>
              <span className="text-on-surface font-mono">
                {info.cpu.frequency_mhz > 0
                  ? `${(info.cpu.frequency_mhz / 1000).toFixed(2)} GHz`
                  : '-'}
              </span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">型号</span>
              <span className="text-on-surface text-[10px] truncate ml-2">{info.cpu.brand}</span>
            </div>
          </div>
        </Card>

        <Card title="内存">
          <div className="space-y-3">
            <div>
              <div className="flex items-baseline justify-between mb-1">
                <span className="text-xs text-on-surface-tertiary">使用率</span>
                <span className={`text-sm font-mono font-medium ${usageColor(memPct)}`}>
                  {memPct.toFixed(1)}%
                </span>
              </div>
              <ProgressBar pct={memPct} />
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-on-surface-tertiary">已用 / 总量</span>
              <span className="text-on-surface font-mono">
                {formatBytes(info.memory.used_bytes)} / {formatBytes(info.memory.total_bytes)}
              </span>
            </div>
            {info.swap.total_bytes > 0 && (
              <div className="flex justify-between text-xs">
                <span className="text-on-surface-tertiary">Swap</span>
                <span className="text-on-surface font-mono">
                  {formatBytes(info.swap.used_bytes)} / {formatBytes(info.swap.total_bytes)}
                </span>
              </div>
            )}
          </div>
        </Card>
      </div>

      {/* 系统负载 */}
      <Card title="系统负载（1 / 5 / 15 分钟）">
        <div className="grid grid-cols-3 gap-3">
          {[
            { label: '1 分钟', value: info.load_average.one },
            { label: '5 分钟', value: info.load_average.five },
            { label: '15 分钟', value: info.load_average.fifteen },
          ].map(({ label, value }) => {
            // 负载相对核心数的百分比：>100% 表示过载
            const loadPct = info.cpu.core_count > 0 ? (value / info.cpu.core_count) * 100 : 0;
            return (
              <div key={label}>
                <div className="flex items-baseline justify-between mb-1">
                  <span className="text-xs text-on-surface-tertiary">{label}</span>
                  <span className={`text-sm font-mono font-medium ${usageColor(loadPct)}`}>
                    {value.toFixed(2)}
                  </span>
                </div>
                <ProgressBar pct={Math.min(loadPct, 100)} />
                <div className="text-[10px] text-on-surface-tertiary mt-1 text-right font-mono">
                  {loadPct.toFixed(0)}% 核心
                </div>
              </div>
            );
          })}
        </div>
      </Card>

      {/* 磁盘 */}
      <Card title="磁盘">
        <div className="space-y-3">
          {info.disks.map((disk, i) => (
            <div key={i}>
              <div className="flex items-baseline justify-between mb-1">
                <span className="text-xs text-on-surface font-mono">{disk.mount_point}</span>
                <span className={`text-xs font-mono ${usageColor(disk.usage_pct)}`}>
                  {disk.usage_pct.toFixed(1)}%
                </span>
              </div>
              <ProgressBar pct={disk.usage_pct} />
              <div className="flex justify-between text-[10px] text-on-surface-tertiary mt-1">
                <span className="font-mono">
                  {formatBytes(disk.used_bytes)} / {formatBytes(disk.total_bytes)}
                </span>
              </div>
            </div>
          ))}
        </div>
      </Card>

      {/* 网络 */}
      <Card title="网络">
        <div className="space-y-2">
          {info.network.length === 0 ? (
            <div className="text-xs text-on-surface-tertiary">无网络接口</div>
          ) : (
            info.network.map((net, i) => {
              const rate = netRates[net.name];
              return (
                <div key={i} className="py-1">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-on-surface font-mono">{net.name}</span>
                    <div className="flex gap-4">
                      <span className="text-on-surface-tertiary">
                        ↓{' '}
                        <span className="text-success-text font-mono">
                          {rate ? formatRate(rate.rx) : '-'}
                        </span>
                      </span>
                      <span className="text-on-surface-tertiary">
                        ↑{' '}
                        <span className="text-info-text font-mono">
                          {rate ? formatRate(rate.tx) : '-'}
                        </span>
                      </span>
                    </div>
                  </div>
                  {net.ips.length > 0 && (
                    <div className="text-[10px] text-on-surface-tertiary mt-0.5 font-mono truncate">
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

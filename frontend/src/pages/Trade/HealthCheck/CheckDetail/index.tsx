import { memo } from 'react';
import { fn } from '@/utils/default';
import { Check, Spinner } from '@/components/Icon';
import type { CheckItem } from '../define';
import ResourceRow from './ResourceRow';

interface Props {
  item: CheckItem;
  onRetry: () => void;
}

const CheckDetail = ({ item, onRetry = fn }: Props) => {
  const { status, data } = item;

  if (status === 'verifying') {
    return <p className="text-[12px] text-on-surface-faint">Checking…</p>;
  }

  if (status === 'error') {
    return (
      <div className="space-y-2">
        <p className="text-[12px] text-danger-text leading-relaxed">
          {data?.message || item.detail}
        </p>
        <button
          onClick={onRetry}
          className="inline-flex items-center gap-1 text-[11px] font-medium text-accent hover:text-accent-hover transition-colors"
        >
          <Spinner className="w-3 h-3" />
          Retry
        </button>
      </div>
    );
  }

  if (status === 'done') {
    switch (item.key) {
      case 'llm':
        return (
          <div className="space-y-2">
            <div className="flex items-center gap-1.5 text-[12px] text-success-text">
              <Check className="w-3.5 h-3.5" strokeWidth={2.5} />
              Reachable
            </div>
            {data?.providers?.length ? (
              <div className="flex flex-wrap gap-1.5">
                {data.providers.map((p) => (
                  <span
                    key={p}
                    className="px-1.5 py-0.5 rounded bg-accent-light text-accent text-[10px] font-medium font-mono"
                  >
                    {p}
                  </span>
                ))}
              </div>
            ) : null}
          </div>
        );
      case 'exchange':
        return (
          <div className="flex items-center gap-1.5 text-[12px] text-success-text">
            <Check className="w-3.5 h-3.5" strokeWidth={2.5} />
            <span className="font-mono">{data?.exchange}</span>
          </div>
        );
      case 'api':
        return (
          <div className="flex items-center gap-1.5 text-[12px]">
            <Check className="w-3.5 h-3.5 text-success-text" strokeWidth={2.5} />
            <span className="text-success-text">Online</span>
            {data?.version && (
              <span className="px-1.5 py-0.5 rounded bg-surface-2 text-on-surface-tertiary text-[10px] font-mono">
                v{data.version}
              </span>
            )}
          </div>
        );
      case 'system':
        return (
          <div className="space-y-2.5">
            <ResourceRow
              label="CPU"
              pct={data?.cpuPct ?? 0}
              hint={`${data?.coreCount ?? 0} cores`}
            />
            <ResourceRow label="Memory" pct={data?.memPct ?? 0} />
            {data?.host && (
              <p className="text-[10px] text-on-surface-muted font-mono truncate pt-0.5">
                {data.host}
              </p>
            )}
          </div>
        );
      case 'trading':
        return (
          <div className="flex items-center gap-2">
            <span
              className={`inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-medium ${
                data?.paperMode
                  ? 'bg-warning-bg text-warning-text'
                  : 'bg-danger-bg text-danger-text'
              }`}
            >
              {data?.paperMode ? 'Paper' : 'Live'}
            </span>
            <span className="text-[12px] text-on-surface-tertiary">
              {data?.paperMode ? 'Simulated execution' : 'Real funds at risk'}
            </span>
          </div>
        );
      default:
        return null;
    }
  }

  return null;
};

export default memo(CheckDetail);

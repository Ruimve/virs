import { memo, type ReactNode } from 'react';

interface PanelSectionProps {
  title: string;
  count?: number;
  children: ReactNode;
  empty?: boolean;
  emptyText?: string;
}

const PanelSection = memo(
  ({ title, count, children, empty, emptyText = '暂无数据' }: PanelSectionProps) => (
    <div className="flex flex-col min-h-0">
      <div className="flex items-center justify-between px-3 py-2 border-b border-line-subtle shrink-0">
        <span className="text-[11px] uppercase tracking-wider text-on-surface-tertiary font-medium">
          {title}
        </span>
        {count !== undefined && (
          <span className="text-[11px] font-mono tabular-nums text-on-surface-muted">{count}</span>
        )}
      </div>
      <div className="flex-1 overflow-y-auto">
        {empty ? (
          <div className="text-center py-6 text-sm text-on-surface-tertiary">{emptyText}</div>
        ) : (
          <div className="divide-y divide-line-subtle">{children}</div>
        )}
      </div>
    </div>
  ),
);

export default PanelSection;

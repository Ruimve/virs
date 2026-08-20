import { memo, type ReactNode } from 'react';
import { Check } from '@/components/Icon';
import { fn } from '@/utils/default';

export type RowType = 'accent' | 'ai';

interface ChoiceRowProps {
  icon: ReactNode;
  title: string;
  description: string;
  badge?: ReactNode;
  selected: boolean;
  type?: RowType;
  onClick: () => void;
}

export const FormRow = memo(
  ({
    icon,
    title,
    description,
    badge,
    selected,
    type = 'accent',
    onClick = fn,
  }: ChoiceRowProps) => {
    const selectedBg = type === 'ai' ? 'bg-ai-light' : 'bg-accent-light';
    const iconSelected = type === 'ai' ? 'bg-ai-light text-ai' : 'bg-accent/15 text-accent';
    const checkSelected = type === 'ai' ? 'border-ai bg-ai' : 'border-accent bg-accent';

    return (
      <div
        onClick={onClick}
        className={`flex items-center gap-3.5 px-4 py-3.5 cursor-pointer transition-colors border-b border-line-subtle last:border-b-0 ${selected ? selectedBg : ''}`}
      >
        <div
          className={`shrink-0 w-10 h-10 rounded-10 flex items-center justify-center transition-all ${selected ? iconSelected : 'bg-surface-2 text-on-surface-muted'}`}
        >
          {icon}
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-sm font-medium text-on-surface-secondary">
            <span className={selected ? 'text-on-base' : ''}>{title}</span>
            {badge}
          </div>
          <p className="text-xs text-on-surface-tertiary mt-0.5 leading-relaxed">{description}</p>
        </div>
        <div
          className={`shrink-0 w-5 h-5 rounded-full border-2 flex items-center justify-center transition-all ${selected ? checkSelected : 'border-line-strong'}`}
        >
          {selected && <Check className="w-3 h-3 text-white" strokeWidth={3} />}
        </div>
      </div>
    );
  },
);

import { memo, type ReactNode } from 'react';
import { Spinner } from '@/components/Icon';

export interface StateFeedbackProps {
  type: 'loading' | 'empty' | 'error' | 'center-loading';
  text?: string;

  icon?: ReactNode;
}

export const StateFeedback = memo(({ type, text, icon }: StateFeedbackProps) => {
  if (type === 'center-loading') {
    return (
      <div className="h-full flex items-center justify-center">
        <Spinner className="h-6 w-6 text-on-surface-tertiary" />
      </div>
    );
  }

  if (type === 'loading') {
    return (
      <div className="flex flex-col items-center justify-center py-16 gap-4 text-on-surface-tertiary text-xs">
        {icon || <Spinner className="h-6 w-6" />}
        <span className="tracking-wider">{text || '加载中'}</span>
      </div>
    );
  }

  if (type === 'empty') {
    return (
      <div className="text-center py-12 text-on-surface-tertiary text-xs">{text || '暂无数据'}</div>
    );
  }

  return (
    <div className="h-full flex items-center justify-center text-danger-text text-sm">
      {text || '发生错误'}
    </div>
  );
});

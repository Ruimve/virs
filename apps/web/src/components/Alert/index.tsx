import { memo, type ReactNode } from 'react';
import { Check, InfoCircle, Warning } from '@/components/Icon';

export type AlertType = 'danger' | 'warning' | 'info' | 'success';

export interface AlertProps {
  type: AlertType;
  title: string;
  icon?: ReactNode;
  className?: string;
  children?: ReactNode;
}

const typeClasses: Record<AlertType, string> = {
  danger: 'bg-danger-bg border-danger-border text-danger-text',
  warning: 'bg-warning-bg border-warning-border text-warning-text',
  info: 'bg-info-bg border-info-border text-info-text',
  success: 'bg-success-bg border-success-border text-success-text',
};

const ICON_CLS = 'w-4 h-4 shrink-0 mt-0.5';

const defaultIcons: Record<AlertType, ReactNode> = {
  danger: <InfoCircle className={ICON_CLS} strokeWidth={1.5} />,
  warning: <Warning className={ICON_CLS} strokeWidth={1.8} />,
  info: <InfoCircle className={ICON_CLS} strokeWidth={1.5} />,
  success: <Check className={ICON_CLS} strokeWidth={2.5} />,
};

export const Alert = memo(({ type, title, icon, className = '', children }: AlertProps) => {
  return (
    <div
      className={`flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg border ${typeClasses[type]} ${className}`}
    >
      {icon || defaultIcons[type]}
      <div className="flex-1 pt-0.5">
        <div className="text-xs tracking-wider">{title}</div>
        {children && <div className=" pt-2.5">{children}</div>}
      </div>
    </div>
  );
});

import { memo, useMemo, type ReactNode } from 'react';

export interface CardProps {
  children: ReactNode;
  title?: string;
  icon?: ReactNode;
  padding?: boolean;
  border?: 'subtle' | 'default' | 'none';
  className?: string;
}

export const Card = memo(
  ({ children, title, icon, padding = true, border = 'default', className = '' }: CardProps) => {
    const borderClass = useMemo(() => {
      switch (border) {
        case 'subtle':
          return 'border-line-subtle border';
        case 'none':
          return '';
        default:
          return 'border-line-default border';
      }
    }, [border]);
    return (
      <div
        className={`bg-surface-1 rounded-xl ${borderClass} ${padding ? 'p-4' : ''} ${className}`}
      >
        {title && (
          <div className="flex items-center gap-2 mb-3">
            {icon && <span className="text-on-surface-tertiary">{icon}</span>}
            <h3 className="text-caption uppercase tracking-caption text-on-surface-tertiary font-medium">
              {title}
            </h3>
          </div>
        )}
        {children}
      </div>
    );
  },
);

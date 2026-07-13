import { memo, type ReactNode } from 'react';

interface CardProps {
  children: ReactNode;
  title?: string;
  icon?: ReactNode;
  padding?: boolean;
  border?: 'subtle' | 'default';
  className?: string;
}

const Card = memo(
  ({ children, title, icon, padding = true, border = 'default', className = '' }: CardProps) => {
    const borderClass = border === 'subtle' ? 'border-line-subtle' : 'border-line-default';
    return (
      <div
        className={`bg-surface-1 rounded-xl ${borderClass} border shadow-sm ${padding ? 'p-4' : ''} ${className}`}
      >
        {title && (
          <div className="flex items-center gap-2 mb-3">
            {icon && <span className="text-on-surface-tertiary">{icon}</span>}
            <h3 className="text-[11px] uppercase tracking-[0.15em] text-on-surface-tertiary font-medium">
              {title}
            </h3>
          </div>
        )}
        {children}
      </div>
    );
  },
);

export default Card;

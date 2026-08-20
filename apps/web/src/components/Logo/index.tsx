import { memo } from 'react';
import { obj, fn } from '@/utils/default';

export { Brand } from './Brand';

export interface LogoProps {
  size?: number;
  className?: string;
  style?: React.CSSProperties;
  onClick?: () => void;
}

export const Logo = memo(({ size = 40, className = '', style = obj, onClick = fn }: LogoProps) => {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={className}
      style={style}
      aria-label="LOGO"
      onClick={onClick}
    >
      <path
        d="M12 2L22 8.5V15.5L12 22L2 15.5V8.5L12 2Z"
        stroke="var(--color-accent)"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      <path d="M12 2L22 8.5L12 15L2 8.5L12 2Z" fill="var(--color-accent)" fillOpacity="0.15" />
      <circle cx="12" cy="9" r="2" fill="var(--color-ai)" />
    </svg>
  );
});

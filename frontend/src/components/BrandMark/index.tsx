import { memo } from 'react';
import { Logo } from '@/components/Logo';
import { fn } from '@/utils/default';

export interface BrandMarkProps {
  size?: number;
  showText?: boolean;
  vertical?: boolean;
  className?: string;
  textClassName?: string;
  onClick?: () => void;
}

/**
 * BrandMark = Logo + "virs" wordmark.
 *
 * Header sizing guide:
 *   - size=20 → h-14 mobile headers (56px)
 *   - size=24 → h-14 desktop headers / sidebar (56px)
 *
 * vertical=true → stack text below logo (for narrow sidebars)
 */
export const BrandMark = memo(
  ({
    size = 24,
    showText = true,
    vertical = false,
    className = '',
    textClassName = '',
    onClick = fn,
  }: BrandMarkProps) => {
    const layout = vertical ? 'flex flex-col items-center gap-0.5' : 'flex items-center gap-1.5';
    const textStyle = vertical
      ? 'font-display font-extralight tracking-widest text-on-base select-none pl-[0.1em] text-2xs'
      : 'font-display font-extralight tracking-caption text-on-base select-none pl-[0.15em]';

    return (
      <div className={`${layout} ${className}`} onClick={onClick}>
        <Logo size={size} />
        {showText && <span className={`${textStyle} ${textClassName}`}>virs</span>}
      </div>
    );
  },
);

BrandMark.displayName = 'BrandMark';

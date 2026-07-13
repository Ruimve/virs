import { forwardRef, memo } from 'react';
import type { IconProps } from '../define';

/**
 * 旋转加载弧（Heroicons outline arrow-path 风格的简化版）。
 * 圆环 opacity-25 + 扇形 opacity-75，配合 animate-spin。
 *
 * Unlike decorative icons, Spinner is NOT aria-hidden because it conveys
 * loading state to screen reader users via role="status".
 */
export const Spinner = memo(
  forwardRef<SVGSVGElement, IconProps>(({ className = '', ...props }, ref) => (
    <svg
      ref={ref}
      className={`animate-spin ${className}`.trim()}
      width={24}
      height={24}
      viewBox="0 0 24 24"
      fill="none"
      role="status"
      aria-label="Loading"
      {...props}
    >
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
      />
    </svg>
  )),
);

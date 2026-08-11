import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

export const Bot = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <rect
        x="4"
        y="8"
        width="16"
        height="12"
        rx="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M12 2v4M9 14h.01M15 14h.01M9 18h6" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx="12" cy="4" r="1" />
    </svg>
  )),
);

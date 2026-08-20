import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

export const User = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
      <circle cx="12" cy="7" r="4" />
    </svg>
  )),
);

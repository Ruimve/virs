import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

export const Flame = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 3c2 4 5 6 5 11 0 4-2 7-5 7s-5-3-5-7c0-2 1-4 3-5 0 1 1 2 2 2 1 0 2-2 0-8z" />
    </svg>
  )),
);

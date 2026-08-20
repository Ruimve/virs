import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

export const Lock = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <rect x="3" y="11" width="18" height="11" rx="2" />
      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
    </svg>
  )),
);

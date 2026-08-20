import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

export const ChevronRight = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M9 5l7 7-7 7" />
    </svg>
  )),
);

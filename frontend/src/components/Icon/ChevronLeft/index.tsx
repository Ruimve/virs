import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

/** 左箭头（返回） */
export const ChevronLeft = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M15 19l-7-7 7-7" />
    </svg>
  )),
);

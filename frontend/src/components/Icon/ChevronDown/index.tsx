import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

/** 下箭头（折叠/展开方向） */
export const ChevronDown = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M19 9l-7 7-7-7" />
    </svg>
  )),
);

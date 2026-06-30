import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

/** 对勾 */
export const Check = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M5 13l4 4L19 7" />
    </svg>
  )),
);

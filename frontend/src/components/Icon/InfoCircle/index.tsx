import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

export const InfoCircle = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  )),
);

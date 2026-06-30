import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

/** 三角警告（Heroicons outline exclamation-triangle） */
export const Warning = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 9v2m0 4h.01M5.07 19h13.86c1.54 0 2.5-1.67 1.73-3L13.73 4c-.77-1.33-2.69-1.33-3.46 0L3.34 16c-.77 1.33.19 3 1.73 3z" />
    </svg>
  )),
);

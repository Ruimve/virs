import { forwardRef, memo } from 'react';
import { strokeBase, type IconProps } from '../define';

/** 盾牌+对勾（安全守护，适合 Paper Trading / 安全模式） */
export const ShieldCheck = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 3l8 3v6c0 5-3.5 8.5-8 10-4.5-1.5-8-5-8-10V6l8-3z" />
      <path d="M9 12l2 2 4-4" />
    </svg>
  )),
);

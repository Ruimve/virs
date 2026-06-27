import { forwardRef, memo, type SVGProps } from 'react';

export type IconProps = SVGProps<SVGSVGElement>;

/** stroke 风格图标的通用 svg 属性 */
const strokeBase = {
  width: 24,
  height: 24,
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
};

/**
 * 旋转加载弧（Heroicons outline arrow-path 风格的简化版）。
 * 圆环 opacity-25 + 扇形 opacity-75，配合 animate-spin。
 */
export const Spinner = memo(
  forwardRef<SVGSVGElement, IconProps>(({ className = '', ...props }, ref) => (
    <svg
      ref={ref}
      className={`animate-spin ${className}`.trim()}
      width={24}
      height={24}
      viewBox="0 0 24 24"
      fill="none"
      {...props}
    >
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
      />
    </svg>
  )),
);
Spinner.displayName = 'Spinner';

/** 对勾 */
export const Check = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M5 13l4 4L19 7" />
    </svg>
  )),
);
Check.displayName = 'Check';

/** 左箭头（返回） */
export const ChevronLeft = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M15 19l-7-7 7-7" />
    </svg>
  )),
);
ChevronLeft.displayName = 'ChevronLeft';

/** 右箭头（展开） */
export const ChevronRight = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M9 5l7 7-7 7" />
    </svg>
  )),
);
ChevronRight.displayName = 'ChevronRight';

/** 下箭头（折叠/展开方向） */
export const ChevronDown = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M19 9l-7 7-7-7" />
    </svg>
  )),
);
ChevronDown.displayName = 'ChevronDown';

/** 关闭 X */
export const Close = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M6 18L18 6M6 6l12 12" />
    </svg>
  )),
);
Close.displayName = 'Close';

/** 三角警告（Heroicons outline exclamation-triangle） */
export const Warning = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 9v2m0 4h.01M5.07 19h13.86c1.54 0 2.5-1.67 1.73-3L13.73 4c-.77-1.33-2.69-1.33-3.46 0L3.34 16c-.77 1.33.19 3 1.73 3z" />
    </svg>
  )),
);
Warning.displayName = 'Warning';

/** 圆圈信息（Heroicons outline exclamation-circle，统一叹号长度） */
export const InfoCircle = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  )),
);
InfoCircle.displayName = 'InfoCircle';

/** 盾牌+对勾（安全守护，适合 Paper Trading / 安全模式） */
export const ShieldCheck = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 3l8 3v6c0 5-3.5 8.5-8 10-4.5-1.5-8-5-8-10V6l8-3z" />
      <path d="M9 12l2 2 4-4" />
    </svg>
  )),
);
ShieldCheck.displayName = 'ShieldCheck';

/** 火焰（真实/能量/风险，适合 Real Trading / 实盘） */
export const Flame = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M12 3c2 4 5 6 5 11 0 4-2 7-5 7s-5-3-5-7c0-2 1-4 3-5 0 1 1 2 2 2 1 0 2-2 0-8z" />
    </svg>
  )),
);
Flame.displayName = 'Flame';

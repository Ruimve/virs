import { forwardRef, memo } from "react";
import { strokeBase, type IconProps } from "../define";

/** 关闭 X */
export const Close = memo(
  forwardRef<SVGSVGElement, IconProps>((props, ref) => (
    <svg ref={ref} {...strokeBase} {...props}>
      <path d="M6 18L18 6M6 6l12 12" />
    </svg>
  )),
);
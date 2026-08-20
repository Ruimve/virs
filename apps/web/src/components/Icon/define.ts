import { type SVGProps } from 'react';

export type IconProps = SVGProps<SVGSVGElement>;

export const strokeBase = {
  width: 24,
  height: 24,
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
};

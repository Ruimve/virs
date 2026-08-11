import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'AssetLoading';

interface Props extends SVGProps<SVGSVGElement> {
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg className="loading-icon" viewBox="0 0 80 80" width={size} height={size} {...rest}>
      <title>Loading assets</title>
      <desc>Minimalist VIRS mark with rotating arcs</desc>

      {}
      <circle className="ring-track" cx="40" cy="40" r="32" />

      {}
      <circle
        className="ring-arc"
        cx="40"
        cy="40"
        r="32"
        pathLength="100"
        strokeDasharray="38 62"
        strokeLinecap="round"
      />

      {}
      <circle
        className="ring-dotted"
        cx="40"
        cy="40"
        r="22"
        pathLength="100"
        strokeDasharray="1.5 6"
      />

      {}
      <path
        className="v-mark"
        d="M27 30 L40 52 L53 30"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
});

import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'aiThinking';

interface Props extends SVGProps<SVGSVGElement> {
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg className="loading-icon" viewBox="0 0 80 80" width={size} height={size} {...rest}>
      <title>AI thinking</title>
      <desc>Minimalist neural network mark with rotating arc</desc>

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
      <g className="neuron-links">
        <line x1="40" y1="40" x2="40" y2="26" strokeLinecap="round" />
        <line x1="40" y1="40" x2="27" y2="48" strokeLinecap="round" />
        <line x1="40" y1="40" x2="53" y2="48" strokeLinecap="round" />
      </g>

      {}
      <circle className="neuron-core" cx="40" cy="40" r="2.4" />

      {}
      <circle className="neuron-node neuron-node-1" cx="40" cy="26" r="1.8" />
      <circle className="neuron-node neuron-node-2" cx="27" cy="48" r="1.8" />
      <circle className="neuron-node neuron-node-3" cx="53" cy="48" r="1.8" />
    </svg>
  );
});

import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'botLoading';

interface Props extends SVGProps<SVGSVGElement> {
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg className="loading-icon" viewBox="0 0 80 80" width={size} height={size} {...rest}>
      <title>Loading bot</title>
      <desc>Minimalist bot mark with rotating arc</desc>

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
      <line className="bot-antenna" x1="40" y1="30" x2="40" y2="23" strokeLinecap="round" />
      <circle className="bot-antenna-dot" cx="40" cy="20" r="1.6" />

      {}
      <rect className="bot-head" x="24" y="30" width="32" height="24" rx="5" ry="5" />

      {}
      <circle className="bot-eye" cx="33" cy="40" r="1.5" />
      <circle className="bot-eye bot-eye-delay" cx="47" cy="40" r="1.5" />

      {}
      <line className="bot-mouth" x1="35" y1="47" x2="45" y2="47" strokeLinecap="round" />
    </svg>
  );
});

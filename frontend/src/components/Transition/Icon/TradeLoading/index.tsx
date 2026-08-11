import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'tradeLoading';

interface Props extends SVGProps<SVGSVGElement> {
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg className="loading-icon" viewBox="0 0 80 80" width={size} height={size} {...rest}>
      <title>Loading trades</title>
      <desc>Minimalist candlestick chart mark with rotating arc</desc>

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
      <line className="candle-wick" x1="30" y1="26" x2="30" y2="46" strokeLinecap="round" />
      <rect className="candle-body candle-body-left" x="27" y="32" width="6" height="9" />

      {}
      <line className="candle-wick" x1="40" y1="20" x2="40" y2="50" strokeLinecap="round" />
      <rect className="candle-body candle-body-center" x="37" y="26" width="6" height="18" />

      {}
      <line className="candle-wick" x1="50" y1="28" x2="50" y2="46" strokeLinecap="round" />
      <rect className="candle-body candle-body-right" x="47" y="33" width="6" height="8" />
    </svg>
  );
});

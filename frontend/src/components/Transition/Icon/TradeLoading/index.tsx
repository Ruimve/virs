/**
 * TradeLoading —— 交易记录加载图标。
 *
 * 视觉：SVG 极简线条——外圈旋转弧（主动律动）+ 静态淡轨道 +
 * 中央 3 根蜡烛 K 线（左短 / 中高 / 右中）。中央蜡烛实体以底部为锚点
 * 呼吸式缩放，暗示"价格波动 / 行情跳动"，贴合交易记录语义。
 * 单一 indigo 主色，自动适配亮/暗主题。
 */

import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'tradeLoading';

interface Props extends SVGProps<SVGSVGElement> {
  /** 图标尺寸（px），默认 56。inline 场景可传 16-20。 */
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg
      className="trade-loading-icon"
      viewBox="0 0 80 80"
      width={size}
      height={size}
      role="img"
      aria-label="Loading trades"
      {...rest}
    >
      <title>Loading trades</title>
      <desc>Minimalist candlestick chart mark with rotating arc</desc>

      {/* 外圈轨道（静态淡环） */}
      <circle className="ring-track" cx="40" cy="40" r="32" />

      {/* 外圈旋转弧 */}
      <circle
        className="ring-arc"
        cx="40"
        cy="40"
        r="32"
        pathLength="100"
        strokeDasharray="38 62"
        strokeLinecap="round"
      />

      {/* 蜡烛 1（左，短，看跌） */}
      <line className="candle-wick" x1="30" y1="26" x2="30" y2="46" strokeLinecap="round" />
      <rect className="candle-body candle-body-left" x="27" y="32" width="6" height="9" />

      {/* 蜡烛 2（中，高，看涨，呼吸） */}
      <line className="candle-wick" x1="40" y1="20" x2="40" y2="50" strokeLinecap="round" />
      <rect className="candle-body candle-body-center" x="37" y="26" width="6" height="18" />

      {/* 蜡烛 3（右，中，看涨） */}
      <line className="candle-wick" x1="50" y1="28" x2="50" y2="46" strokeLinecap="round" />
      <rect className="candle-body candle-body-right" x="47" y="33" width="6" height="8" />
    </svg>
  );
});

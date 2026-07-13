/**
 * BotLoading —— Bot 加载过渡图标。
 *
 * 视觉：SVG 极简线条构成——外圈旋转弧（主动律动）+ 静态淡轨道 +
 * 中央 Bot 头部线条标（圆角矩形头 + 天线 + 双眼 + 嘴部栅格）。
 * 天线顶端圆点脉冲式闪烁，暗示"接收信号 / 思考中"，贴合 AI Bot 语义。
 * 单一 indigo 主色，自动适配亮/暗主题。
 */

import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'botLoading';

interface Props extends SVGProps<SVGSVGElement> {
  /** 图标尺寸（px），默认 56。inline 场景可传 16-20。 */
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg className="bot-loading-icon" viewBox="0 0 80 80" width={size} height={size} {...rest}>
      <title>Loading bot</title>
      <desc>Minimalist bot mark with rotating arc</desc>

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

      {/* 天线 */}
      <line className="bot-antenna" x1="40" y1="30" x2="40" y2="23" strokeLinecap="round" />
      <circle className="bot-antenna-dot" cx="40" cy="20" r="1.6" />

      {/* Bot 头部（圆角矩形） */}
      <rect className="bot-head" x="24" y="30" width="32" height="24" rx="5" ry="5" />

      {/* 双眼 */}
      <circle className="bot-eye" cx="33" cy="40" r="1.5" />
      <circle className="bot-eye bot-eye-delay" cx="47" cy="40" r="1.5" />

      {/* 嘴部栅格（暗示策略/数据） */}
      <line className="bot-mouth" x1="35" y1="47" x2="45" y2="47" strokeLinecap="round" />
    </svg>
  );
});

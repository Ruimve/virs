/**
 * AiThinking —— AI 决策/分析加载图标。
 *
 * 视觉：SVG 极简线条——外圈旋转弧（主动律动）+ 静态淡轨道 +
 * 中央神经网络节点拓扑（1 中央节点 + 3 外围节点 + 连线）。
 * 外围节点错时脉冲闪烁，暗示"信号传递 / 推理中"，贴合 AI 决策语义。
 * 单一 indigo 主色，自动适配亮/暗主题。
 */

import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'aiThinking';

interface Props extends SVGProps<SVGSVGElement> {
  /** 图标尺寸（px），默认 56。inline 场景可传 16-20。 */
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg
      className="ai-thinking-icon"
      viewBox="0 0 80 80"
      width={size}
      height={size}
      role="img"
      aria-label="AI thinking"
      {...rest}
    >
      <title>AI thinking</title>
      <desc>Minimalist neural network mark with rotating arc</desc>

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

      {/* 神经网络连线（先画，置底层） */}
      <g className="neuron-links">
        <line x1="40" y1="40" x2="40" y2="26" strokeLinecap="round" />
        <line x1="40" y1="40" x2="27" y2="48" strokeLinecap="round" />
        <line x1="40" y1="40" x2="53" y2="48" strokeLinecap="round" />
      </g>

      {/* 中央节点 */}
      <circle className="neuron-core" cx="40" cy="40" r="2.4" />

      {/* 外围节点（错时脉冲） */}
      <circle className="neuron-node neuron-node-1" cx="40" cy="26" r="1.8" />
      <circle className="neuron-node neuron-node-2" cx="27" cy="48" r="1.8" />
      <circle className="neuron-node neuron-node-3" cx="53" cy="48" r="1.8" />
    </svg>
  );
});

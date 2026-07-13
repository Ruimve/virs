/**
 * LlmLoading —— LLM 配置/验证加载图标。
 *
 * 视觉：SVG 极简线条——外圈旋转弧（主动律动）+ 静态淡轨道 +
 * 中央芯片核心（描边圆 + 实心内核点）+ 4 对角放射线（信号辐射）。
 * 内核点脉冲缩放，4 对角线错时闪烁，暗示"模型推理 / 信号扩散"，
 * 贴合 LLM 验证 / 推理调用语义。单一 indigo 主色，自动适配亮/暗主题。
 */

import { memo, type SVGProps } from 'react';
import './style.css';

export type IconName = 'llmLoading';

interface Props extends SVGProps<SVGSVGElement> {
  /** 图标尺寸（px），默认 56。inline 场景可传 16-20。 */
  size?: number;
}

export const Icon = memo((props: Props) => {
  const { size = 56, ...rest } = props;
  return (
    <svg
      className="llm-loading-icon"
      viewBox="0 0 80 80"
      width={size}
      height={size}
      role="img"
      aria-label="Loading LLM"
      {...rest}
    >
      <title>Loading LLM</title>
      <desc>Minimalist chip core mark with rotating arc</desc>

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

      {/* 4 对角放射线（先画，置底层） */}
      <g className="chip-rays">
        <line
          className="chip-ray chip-ray-1"
          x1="32"
          y1="32"
          x2="26"
          y2="26"
          strokeLinecap="round"
        />
        <line
          className="chip-ray chip-ray-2"
          x1="48"
          y1="32"
          x2="54"
          y2="26"
          strokeLinecap="round"
        />
        <line
          className="chip-ray chip-ray-3"
          x1="32"
          y1="48"
          x2="26"
          y2="54"
          strokeLinecap="round"
        />
        <line
          className="chip-ray chip-ray-4"
          x1="48"
          y1="48"
          x2="54"
          y2="54"
          strokeLinecap="round"
        />
      </g>

      {/* 中央芯片核心（描边圆） */}
      <circle className="chip-core" cx="40" cy="40" r="6" />

      {/* 内核点（实心，脉冲） */}
      <circle className="chip-inner" cx="40" cy="40" r="2" />
    </svg>
  );
});

/**
 * LoadAssets —— 过渡状态图标。
 *
 * 视觉：SVG 极简线条构成的加载态——外圈旋转弧（主动律动）+ 内圈点状环
 * （次级反向慢转）+ 中央 V 字标（呼吸式 opacity）。三层视觉层级清晰，
 * 单一 indigo 主色，自动适配亮/暗主题。
 */

import './style.css';

export type IconName = 'loadingAssets';

const Icon = () => {
  return (
    <svg className="load-assets-icon" viewBox="0 0 80 80" role="img" aria-label="Loading assets">
      <title>Loading assets</title>
      <desc>Minimalist VIRS mark with rotating arcs</desc>

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

      {/* 内圈点状环 */}
      <circle
        className="ring-dotted"
        cx="40"
        cy="40"
        r="22"
        pathLength="100"
        strokeDasharray="1.5 6"
      />

      {/* 中央 V 字标 */}
      <path
        className="v-mark"
        d="M27 30 L40 52 L53 30"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
};

export { Icon };

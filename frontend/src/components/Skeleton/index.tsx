/**
 * Skeleton —— 加载态骨架占位。
 *
 * 职责：在数据加载完成前渲染与最终内容同尺寸的占位块，保持 DOM 结构
 * 稳定，避免加载完成时因内容尺寸/结构变化导致的页面回流重绘。
 *
 * 用法：
 *  <Skeleton className="h-3 w-16" />        // 文本占位
 *  <Skeleton className="h-4 w-24 rounded" /> // 自定义
 */
import { memo } from 'react';

interface Props {
  className?: string;
  /** 圆角风格，默认 text（小圆角适合文本），可选 block（大圆角适合卡片块） */
  variant?: 'text' | 'block';
}

const Skeleton = ({ className = '', variant = 'text' }: Props) => {
  const radius = variant === 'text' ? 'rounded' : 'rounded-md';
  return (
    <div
      className={`bg-surface-2/70 ${radius} animate-pulse ${className}`}
      role="presentation"
      aria-hidden="true"
    />
  );
};

export default memo(Skeleton);

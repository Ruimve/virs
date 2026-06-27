/**
 * Transition —— 全局过渡状态布局。
 *
 * 职责：提供全屏居中的过渡态容器，通过 `icon` prop 选择过渡态图标。
 * 紧凑设计，仅居中渲染图标，无额外装饰。
 *
 * 用法：<Transition icon="loadingAssets" />
 */
import { memo, type ReactNode } from 'react';
import { iconMap, type TransitionIcon } from './Icon';

interface TransitionProps {
  icon?: TransitionIcon;
  children?: ReactNode;
}

const Transition = ({ icon, children }: TransitionProps) => {
  const Icon = icon ? iconMap[icon] : null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-base"
      role="status"
      aria-live="polite"
    >
      <div className="flex flex-col items-center">{Icon && <Icon />}</div>
      {children}
    </div>
  );
};

export default memo(Transition);

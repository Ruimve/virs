import { createContext, useContext, type ReactNode } from 'react';

export interface NavItem {
  key: string;
  label: string;
  icon: ReactNode;
  onClick: () => void;
}

export interface ActionItem {
  key: string;
  label: string;
  className?: string;
  onClick: () => void;
}

export interface ShellContextValue {
  navItems: NavItem[];
  setNavItems: (items: NavItem[]) => void;
  actions: ActionItem[];
  setActions: (actions: ActionItem[]) => void;
  activeNav: string;
  openDrawer: () => void;
}

export const ShellContext = createContext<ShellContextValue | null>(null);

export const useShell = () => {
  const context = useContext(ShellContext);
  if (!context) {
    throw new Error('useShell 必须在 ShellContext 内部使用');
  }
  return context;
};

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

export interface LayoutContextValue {
  navItems: NavItem[];
  setNavItems: (items: NavItem[]) => void;
  actions: ActionItem[];
  setActions: (actions: ActionItem[]) => void;
  activeNav: string;
  drawerOpen: boolean;
  changeDrawerOpen: (open: boolean) => void;
}

export const LayoutContext = createContext<LayoutContextValue | null>(null);

export const useLayout = () => {
  const context = useContext(LayoutContext);
  if (!context) {
    throw new Error('useLayout 必须在 LayoutContext 内部使用');
  }
  return context;
};

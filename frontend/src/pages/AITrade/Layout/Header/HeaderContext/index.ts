import { createContext, useContext } from 'react';

export type ItemConfig = {
  key: string;
  label: string;
  className?: string;
  onClick: (key: string) => void;
};

export interface HeaderContextType {
  left: ItemConfig[];
  tabs: ItemConfig[];
  actions: ItemConfig[];
  activeTab?: string;
  updateLeft: (tabs: ItemConfig[]) => void;
  updateTabs: (tabs: ItemConfig[]) => void;
  updateActions: (tabs: ItemConfig[]) => void;
}

export const DEFAULT_HEADER: HeaderContextType = {
  left: [],
  tabs: [],
  actions: [],
  updateLeft: () => void 0,
  updateTabs: () => void 0,
  updateActions: () => void 0,
};

export const HeaderContext = createContext<HeaderContextType>(DEFAULT_HEADER);

export const useHeader = () => {
  const context = useContext(HeaderContext);
  if (!context) {
    throw new Error('useHeader 必须在 HeaderProvider 内部使用');
  }
  return context;
};

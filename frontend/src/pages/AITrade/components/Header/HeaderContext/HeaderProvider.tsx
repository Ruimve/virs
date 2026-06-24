import { useMemo, useState, type ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import { HeaderContext, type ItemConfig } from '.';

export const HeaderProvider = ({ children }: { children: ReactNode }) => {
  const location = useLocation();
  const [left, setLeft] = useState<ItemConfig[]>([]);
  const [tabs, setTabs] = useState<ItemConfig[]>([]);
  const [actions, setActions] = useState<ItemConfig[]>([]);

  const activeTab = useMemo(() => {
    const paths = location.pathname.split('/') || [];
    return paths[paths.length - 1];
  }, [location.pathname]);

  return (
    <HeaderContext.Provider
      value={{
        left,
        tabs,
        actions,
        activeTab,
        updateLeft: setLeft,
        updateTabs: setTabs,
        updateActions: setActions,
      }}
    >
      {children}
    </HeaderContext.Provider>
  );
};

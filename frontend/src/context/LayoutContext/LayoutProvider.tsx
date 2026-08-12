import { useState, type ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import { LayoutContext, type ActionItem, type NavItem } from '.';

export const LayoutProvider = ({ children }: { children: ReactNode }) => {
  const location = useLocation();
  const [navItems, setNavItems] = useState<NavItem[]>([]);
  const [actions, setActions] = useState<ActionItem[]>([]);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const activeNav = (() => {
    const paths = location.pathname.split('/') || [];
    return paths[paths.length - 1];
  })();

  return (
    <LayoutContext.Provider
      value={{
        navItems,
        setNavItems,
        actions,
        setActions,
        activeNav,
        drawerOpen,
        changeDrawerOpen: setDrawerOpen,
      }}
    >
      {children}
    </LayoutContext.Provider>
  );
};

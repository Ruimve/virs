import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';
import { useLocation } from 'react-router-dom';

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

const DEFAULT_VALUE: ShellContextValue = {
  navItems: [],
  setNavItems: () => {},
  actions: [],
  setActions: () => {},
  activeNav: '',
  openDrawer: () => {},
};

export const ShellContext = createContext<ShellContextValue>(DEFAULT_VALUE);

export const useShell = () => {
  const ctx = useContext(ShellContext);
  return ctx;
};

export const ShellProvider = ({ children }: { children: ReactNode }) => {
  const [navItems, setNavItems] = useState<NavItem[]>([]);
  const [actions, setActions] = useState<ActionItem[]>([]);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const location = useLocation();

  const activeNav = (() => {
    const paths = location.pathname.split('/') || [];
    return paths[paths.length - 1];
  })();

  const openDrawer = useCallback(() => setDrawerOpen(true), []);
  const closeDrawer = useCallback(() => setDrawerOpen(false), []);

  return (
    <ShellContext.Provider
      value={{
        navItems,
        setNavItems,
        actions,
        setActions,
        activeNav,
        openDrawer,
      }}
    >
      <ShellDrawer
        open={drawerOpen}
        onClose={closeDrawer}
        navItems={navItems}
        activeNav={activeNav}
      />
      {children}
    </ShellContext.Provider>
  );
};

// Mobile drawer — rendered by provider so any header can trigger it via openDrawer()
const ShellDrawer = ({
  open,
  onClose,
  navItems,
  activeNav,
}: {
  open: boolean;
  onClose: () => void;
  navItems: NavItem[];
  activeNav: string;
}) => {
  if (!open || navItems.length === 0) return null;

  return (
    <div className="fixed inset-0 z-50 md:hidden" onClick={onClose}>
      <div className="absolute inset-0 bg-base/60 backdrop-blur-sm" />
      <div
        className="absolute inset-y-0 left-0 w-64 bg-base border-r border-line-subtle flex flex-col animate-drawer-in"
        onClick={(e) => e.stopPropagation()}
      >
        <nav className="flex-1 overflow-y-auto py-2">
          {navItems.map((item) => (
            <button
              key={item.key}
              type="button"
              onClick={() => {
                item.onClick();
                onClose();
              }}
              className={`w-full flex items-center gap-3 px-5 py-2.5 text-xs font-medium tracking-wide transition-colors cursor-pointer ${
                activeNav === item.key
                  ? 'text-accent bg-accent-light border-l-2 border-accent'
                  : 'text-on-surface-secondary hover:bg-surface-2/50 hover:text-on-surface'
              }`}
            >
              <span className="w-5 h-5 flex items-center justify-center">{item.icon}</span>
              {item.label}
            </button>
          ))}
        </nav>
      </div>
    </div>
  );
};

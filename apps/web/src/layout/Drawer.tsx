import { useLayout } from '../context/LayoutContext';

export const Drawer = () => {
  const { drawerOpen, changeDrawerOpen, navItems, activeNav } = useLayout();

  if (!drawerOpen || navItems.length === 0) return null;

  return (
    <div className="fixed inset-0 z-50 md:hidden" onClick={() => changeDrawerOpen(false)}>
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
                changeDrawerOpen(false);
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

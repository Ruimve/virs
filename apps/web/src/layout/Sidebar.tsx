import { memo } from 'react';
import { Brand } from '@/components/Logo';
import { useLayout } from '@/context/LayoutContext';

export const Sidebar = memo(() => {
  const { navItems, activeNav } = useLayout();

  if (navItems.length === 0) return null;

  return (
    <aside className="hidden md:flex flex-col items-center w-14 h-full bg-surface-1 border-r border-line-default shrink-0">
      <div className="mt-3 mb-4 shrink-0">
        <Brand size={24} vertical />
      </div>

      <nav className="flex flex-col items-center gap-1 flex-1 w-full">
        {navItems.map((item) => {
          const isActive = activeNav === item.key;
          return (
            <button
              key={item.key}
              type="button"
              title={item.label}
              onClick={item.onClick}
              className={`relative flex items-center justify-center w-10 h-10 rounded-10 transition-colors duration-200 ${
                isActive
                  ? 'bg-accent-light text-accent'
                  : 'text-on-surface-tertiary hover:text-on-surface hover:bg-surface-2'
              }`}
            >
              {isActive && (
                <span className="absolute -left-2 top-1/2 -translate-y-1/2 w-0.75 h-5 rounded-full bg-accent" />
              )}
              {item.icon}
            </button>
          );
        })}
      </nav>
    </aside>
  );
});

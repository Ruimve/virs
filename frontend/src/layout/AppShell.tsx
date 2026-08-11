import { memo, type ReactNode } from 'react';
import { BrandMark } from '@/components/BrandMark';
import { useShell } from './ShellContext';

interface AppShellProps {
  header: ReactNode;
  children: ReactNode;
  sidebar?: boolean;
}

const ShellSidebar = memo(() => {
  const { navItems, activeNav } = useShell();

  if (navItems.length === 0) return null;

  return (
    <aside className="hidden md:flex flex-col items-center w-14 h-full bg-surface-1 border-r border-line-default shrink-0">
      <div className="mt-3 mb-4 shrink-0">
        <BrandMark size={24} vertical />
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
                <span className="absolute -left-2 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full bg-accent" />
              )}
              {item.icon}
            </button>
          );
        })}
      </nav>
    </aside>
  );
});

export const AppShell = memo(({ header, children, sidebar = true }: AppShellProps) => {
  return (
    <div className="relative flex h-dvh overflow-hidden bg-base">
      {sidebar && <ShellSidebar />}

      <div className="flex flex-1 flex-col overflow-hidden">
        <header>{header}</header>
        <main className="h-0 flex-1 overflow-hidden">{children}</main>
      </div>
    </div>
  );
});

export default AppShell;

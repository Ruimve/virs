import { memo, type ReactNode } from 'react';
import { Sidebar } from './Sidebar';
import { LayoutProvider } from '@/context/LayoutContext/LayoutProvider';
import { Drawer } from './Drawer';

interface LayoutProps {
  header: ReactNode;
  children: ReactNode;
  sidebar?: boolean;
}

export const Layout = memo(({ header, children, sidebar = true }: LayoutProps) => {
  return (
    <LayoutProvider>
      <Drawer />
      <div className="relative flex h-dvh overflow-hidden bg-base">
        {sidebar && <Sidebar />}

        <div className="flex flex-1 flex-col overflow-hidden">
          <header>{header}</header>
          <main className="h-0 flex-1 overflow-hidden">{children}</main>
        </div>
      </div>
    </LayoutProvider>
  );
});

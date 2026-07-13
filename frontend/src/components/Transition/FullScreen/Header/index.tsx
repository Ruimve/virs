import { Logo } from '@/components/Logo';
import { Theme } from '@/components/Theme';

export const Header = () => (
  <header className="fixed top-0 left-0 right-0 h-14 md:h-16 z-10 bg-base/80 backdrop-blur-md border-b border-line-subtle">
    <div className="flex items-center justify-between h-full px-4">
      <Logo />

      <div className="flex items-center gap-2">
        <Theme />
      </div>
    </div>
  </header>
);

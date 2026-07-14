import { Logo } from '@/components/Logo';
import { Theme } from '@/components/Theme';

export const Header = () => {
  return (
    <div className="relative z-10 flex items-center h-12 md:h-12 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      {}
      <div className="flex items-center gap-2.5 pl-3 md:pl-6 shrink-0">
        <Logo />
      </div>

      {}
      <div className="hidden md:flex items-center justify-center flex-1 gap-0.5"></div>

      {}
      <div className="hidden md:flex items-center gap-1.5 pr-6 shrink-0">
        <Theme />
      </div>

      {}
      <div className="md:hidden flex items-center pr-3 ml-auto shrink-0">
        <Theme />
      </div>
    </div>
  );
};

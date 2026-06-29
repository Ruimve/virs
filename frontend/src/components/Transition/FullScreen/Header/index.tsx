import Logo from '@/components/Logo';
import Theme from '@/components/Theme';

const Header = () => {
  return (
    <div className="relative z-10 flex items-center h-14 md:h-16 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      {/* Left: logo (clickable on mobile to open drawer) + bot info */}
      <div className="flex items-center gap-2 pl-4 md:pl-8 shrink-0">
        <Logo />
      </div>

      {/* Center: tabs (desktop) */}
      <div className="hidden md:flex items-center justify-center flex-1 gap-1"></div>

      {/* Right: actions (desktop) */}
      <div className="hidden md:flex items-center gap-2 pr-8 shrink-0">
        <Theme />
      </div>

      {/* Right: theme toggle (mobile) */}
      <div className="md:hidden flex items-center pr-4 ml-auto shrink-0">
        <Theme />
      </div>
    </div>
  );
};

export default Header;

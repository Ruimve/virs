import { BrandMark } from '@/components/BrandMark';
import { Theme } from '@/components/Theme';

export const Header = () => {
  return (
    <div className="relative z-10 flex items-center h-14 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      <div className="pl-3 md:pl-4 shrink-0">
        <div className="md:hidden">
          <BrandMark size={20} />
        </div>
        <div className="hidden md:block">
          <BrandMark size={24} />
        </div>
      </div>

      <div className="flex items-center gap-1.5 pr-3 md:pr-4 ml-auto shrink-0">
        <Theme />
      </div>
    </div>
  );
};

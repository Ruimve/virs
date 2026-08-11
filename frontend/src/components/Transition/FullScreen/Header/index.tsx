import { BrandMark } from '@/components/BrandMark';
import { Theme } from '@/components/Theme';

export const Header = () => {
  return (
    <div className="relative z-10 flex items-center h-12 border-b border-line-subtle bg-base/80 backdrop-blur-xl">
      <div className="pl-3 md:pl-6 shrink-0">
        <BrandMark size={24} />
      </div>

      <div className="flex items-center gap-1.5 pr-3 md:pr-6 ml-auto shrink-0">
        <Theme />
      </div>
    </div>
  );
};

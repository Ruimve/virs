import { memo, type ReactNode } from 'react';
import { Header } from './Header';

interface Props {
  header?: boolean;
  icon: ReactNode;
}
export const FullScreen = memo((props: Props) => {
  const { header = false, icon } = props;
  return (
    <div className="h-dvh bg-base flex flex-col relative overflow-hidden">
      {header && <Header />}
      <div
        className={`flex-1 flex items-center justify-center ${header ? '-mt-14 md:-mt-16' : ''}`}
      >
        {icon}
      </div>
    </div>
  );
});

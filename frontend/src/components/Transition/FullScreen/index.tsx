import { memo, type ReactNode } from 'react';
import Header from './Header';

interface Props {
  header?: boolean;
  icon: ReactNode;
}
const FullScreen = (props: Props) => {
  const { header = false, icon } = props;
  return (
    <div className="h-dvh bg-base flex flex-col relative overflow-hidden">
      {header && <Header />}
      <div className="flex-1 flex flex-col items-center">{icon}</div>
    </div>
  );
};

export default memo(FullScreen);

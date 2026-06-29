import { memo, type ReactNode } from 'react';

interface Props {
  header: ReactNode;
  children?: ReactNode;
}
const Layout = (props: Props) => {
  const { header, children } = props;
  return (
    <div className="h-dvh bg-base flex flex-col relative overflow-hidden">
      {/* Header */}
      <header>{header}</header>

      {/* Content */}
      <main className="flex-1 h-0">{children}</main>
    </div>
  );
};

export default memo(Layout);

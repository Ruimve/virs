import { Outlet } from 'react-router-dom';
import Header from '../components/Header';

export const Layout = () => {
  return (
    <div className="h-screen bg-base flex flex-col relative overflow-hidden">
      <div className="absolute inset-0 overflow-hidden">
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full bg-accent/3 blur-[120px]" />
      </div>
      <Header />
      <div className="flex-1 overflow-y-auto relative z-10">
        <Outlet />
      </div>
    </div>
  );
};

export default Layout;

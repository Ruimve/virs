import { Outlet } from 'react-router-dom';
import GlobalLayout from '@/layout';
import Header from './Header';

export const Layout = () => {
  return (
    <GlobalLayout header={<Header />}>
      <Outlet />
    </GlobalLayout>
  );
};

export default Layout;

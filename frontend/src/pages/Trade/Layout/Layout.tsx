import { Outlet } from 'react-router-dom';
import Header from './Header';
import GlobalLayout from '@/layout';

export const Layout = () => {
  return (
    <GlobalLayout header={<Header />}>
      <Outlet />
    </GlobalLayout>
  );
};

export default Layout;

import { Outlet } from 'react-router-dom';
import { AuthProvider } from '@/context/AuthContext/AuthProvider';

const Guard = () => {
  return (
    <AuthProvider>
      <Outlet />
    </AuthProvider>
  );
};

export default Guard;

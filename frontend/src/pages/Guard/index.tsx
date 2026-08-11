import { use, useMemo } from 'react';
import { Navigate, Outlet } from 'react-router-dom';
import { getUser } from '@/context/AuthContext/auth';
import { AuthContext } from '@/context/AuthContext';

const promiseUser = getUser();

const Guard = () => {
  const user = use(promiseUser);
  const value = useMemo(() => ({ user }), [user]);

  if (!user) {
    return <Navigate to="/login" replace />;
  }
  return (
    <AuthContext.Provider value={value}>
      <Outlet />
    </AuthContext.Provider>
  );
};

export default Guard;

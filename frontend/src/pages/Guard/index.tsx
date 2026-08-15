import { Suspense, useState } from 'react';
import { Outlet } from 'react-router-dom';
import { AuthProvider } from '@/context/AuthContext/AuthProvider';
import Fallback from '@/components/Transition/Fallback';
import { getUser } from '@/context/AuthContext/auth';

const Guard = () => {
  const [promiseUser] = useState(() => getUser());
  return (
    <Suspense fallback={<Fallback label="正在加载用户配置" startProgress={45} progress={85} />}>
      <AuthProvider promiseUser={promiseUser}>
        <Outlet />
      </AuthProvider>
    </Suspense>
  );
};

export default Guard;

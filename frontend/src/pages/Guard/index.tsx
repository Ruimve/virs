import { Suspense, useState } from 'react';
import { Outlet } from 'react-router-dom';
import { AuthProvider } from '@/context/AuthContext/AuthProvider';
import Fallback from '@/components/Transition/Fallback';
import { getUser } from '@/context/AuthContext/auth';

const Guard = () => {
  const [promiseUser] = useState(() => getUser());

  return (
    <Suspense fallback={<Fallback label="正在检查账户..." startProgress={35} progress={60} />}>
      <AuthProvider promiseUser={promiseUser}>
        <Suspense fallback={<Fallback label="正在加载页面..." startProgress={65} progress={90} />}>
          <Outlet />
        </Suspense>
      </AuthProvider>
    </Suspense>
  );
};

export default Guard;

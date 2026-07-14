import { useEffect, useMemo, use, Suspense, type ReactNode } from 'react';
import { Navigate, useNavigate } from 'react-router-dom';
import { removeToken } from '@/service';
import { FullScreen } from '@/components/Transition/FullScreen';
import { AssetLoading } from '@/components/Transition/Icon';
import { AUTH_UNAUTHORIZED_EVENT, AuthContext } from './';
import { getUser } from './auth';

const promiseUser = getUser();

export function AuthProviderMain({ children }: { children: ReactNode }) {
  const user = use(promiseUser);
  const value = useMemo(() => ({ user }), [user]);

  if (value.user) {
    return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
  }
  return <Navigate to="/login" replace />;
}

export const AuthProvider = ({ children }: { children: ReactNode }) => {
  const navigate = useNavigate();

  useEffect(() => {
    const handleUnauthorized = () => {
      removeToken();
      navigate('/login', { replace: true });
    };
    window.addEventListener(AUTH_UNAUTHORIZED_EVENT, handleUnauthorized);
    return () => window.removeEventListener(AUTH_UNAUTHORIZED_EVENT, handleUnauthorized);
  }, [navigate]);

  return (
    <Suspense fallback={<FullScreen icon={<AssetLoading />} />}>
      <AuthProviderMain>{children}</AuthProviderMain>
    </Suspense>
  );
};

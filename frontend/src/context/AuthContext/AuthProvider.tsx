import { useEffect, useMemo, use, type ReactNode } from 'react';
import { Navigate, useNavigate } from 'react-router-dom';
import { removeToken, type UserInfo } from '@/service';
import { AUTH_UNAUTHORIZED_EVENT, AuthContext } from './';

export const AuthProvider = ({
  promiseUser,
  children,
}: {
  promiseUser: Promise<UserInfo | null>;
  children: ReactNode;
}) => {
  const navigate = useNavigate();

  useEffect(() => {
    const handleUnauthorized = () => {
      removeToken();
      navigate('/login', { replace: true });
    };
    window.addEventListener(AUTH_UNAUTHORIZED_EVENT, handleUnauthorized);
    return () => window.removeEventListener(AUTH_UNAUTHORIZED_EVENT, handleUnauthorized);
  }, [navigate]);

  const user = use(promiseUser);
  const value = useMemo(() => ({ user }), [user]);

  if (!value.user) return <Navigate to="/login" replace />;
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};

/**
 * 认证 Provider 与路由守卫组件（仅导出组件，规避 react-refresh 警告）
 *
 * 设计要点：
 *  1. Context + Provider 管理状态，与项目现有 WizardProvider/BotProvider 风格一致
 *  2. ProtectedRoute 组件做路由守卫，替代 requireAuth 的命令式跳转
 *  3. 401 拦截留在 client.ts，通过 'auth:unauthorized' 事件通知 Provider 清空状态并导航
 *  4. Provider 置于 BrowserRouter 内部，可用 useNavigate 做声明式跳转
 *  5. 消除 forceUpdate / 手动 subscribe / notify 的样板代码，规避 Concurrent tearing 风险
 */
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

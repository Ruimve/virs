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
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router-dom';
import {
  type UserInfo,
  getUserInfo,
  login as loginService,
  getToken,
  setToken,
  removeToken,
} from '@/service';
import {
  type AuthContextType,
  type LoginResult,
  AUTH_UNAUTHORIZED_EVENT,
  AuthContext,
  useAuth,
} from './';

export function AuthProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const [user, setUser] = useState<UserInfo | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    if (!getToken()) {
      setUser(null);
      // 无 token 时也要结束 loading，否则守卫会一直返回 null 卡在空白页
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      const result = await getUserInfo();
      if (result.success && result.data) {
        setUser(result.data);
        return;
      }
      setUser(null);
      removeToken();
    } catch {
      setUser(null);
      removeToken();
    } finally {
      setLoading(false);
    }
  }, []);

  // 应用启动时恢复会话
  useEffect(() => {
    refresh();
  }, [refresh]);

  // 监听 401 失效事件：清空状态并跳转登录页（替代裸 window.location）
  useEffect(() => {
    const handleUnauthorized = () => {
      setUser(null);
      removeToken();
      navigate('/login', { replace: true });
    };
    window.addEventListener(AUTH_UNAUTHORIZED_EVENT, handleUnauthorized);
    return () => window.removeEventListener(AUTH_UNAUTHORIZED_EVENT, handleUnauthorized);
  }, [navigate]);

  const login = useCallback(
    async (username: string, password: string): Promise<LoginResult> => {
      const result = await loginService(username, password);
      if (result.success && result.data) {
        setToken(result.data.token);
        await refresh();
        return { success: true };
      }
      return { success: false, error: result.error || 'Login failed' };
    },
    [refresh],
  );

  const logout = useCallback(() => {
    setUser(null);
    removeToken();
    navigate('/login', { replace: true });
  }, [navigate]);

  const value = useMemo<AuthContextType>(
    () => ({ user, loading, login, logout, refresh }),
    [user, loading, login, logout, refresh],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

/** 路由守卫组件，替代 requireAuth 命令式跳转 */
export function AuthProtecter({ children }: { children: ReactNode }) {
  const { user, loading } = useAuth();
  const location = useLocation();

  if (loading) {
    return null;
  }
  if (!user) {
    return <Navigate to="/login" replace state={{ from: location }} />;
  }
  return <>{children}</>;
}

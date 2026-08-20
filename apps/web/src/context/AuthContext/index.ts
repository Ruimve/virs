import { createContext, useContext } from 'react';
import type { UserInfo } from '@/service';

export const AUTH_UNAUTHORIZED_EVENT = 'auth:unauthorized';

export interface AuthContextType {
  user: UserInfo | null;
}

export const AuthContext = createContext<AuthContextType | null>(null);

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useBot 必须在 AuthContext 内部使用');
  }
  return context;
};

import { createContext, useContext } from 'react';
import type { UserInfo } from '@/service';

export const AUTH_UNAUTHORIZED_EVENT = 'auth:unauthorized';

export interface AuthContextType {
  user: UserInfo | null;
}

export const AuthContext = createContext<AuthContextType | null>(null);

export function useAuth(): AuthContextType {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error('useAuth 必须在 AuthProvider 内部使用');
  }
  return ctx;
}

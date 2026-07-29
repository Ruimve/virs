import { createContext } from 'react';
import type { UserInfo } from '@/service';

export const AUTH_UNAUTHORIZED_EVENT = 'auth:unauthorized';

export interface AuthContextType {
  user: UserInfo | null;
}

export const AuthContext = createContext<AuthContextType | null>(null);

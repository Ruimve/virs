import { createContext, useContext } from 'react';

interface PaperContextType {
  enabled: boolean | null;
  refresh: () => Promise<void>;
}

export const PaperContext = createContext<PaperContextType | null>(null);

export function usePaper() {
  const ctx = useContext(PaperContext);
  if (!ctx) throw new Error('usePaper 必须在 PaperProvider 内部使用');
  return ctx;
}

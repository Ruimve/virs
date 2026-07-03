import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { getPaperStatus } from '@/service';
import { PaperContext } from '.';

export function PaperProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabled] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const res = await getPaperStatus();
      if (res.success && res.data) setEnabled(res.data.paper_mode);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <PaperContext.Provider value={{ enabled, refresh }}>
      {children}
    </PaperContext.Provider>
  );
}

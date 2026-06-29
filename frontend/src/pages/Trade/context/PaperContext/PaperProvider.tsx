import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { disablePaperMode, enablePaperMode, getPaperStatus } from '@/service';
import { PaperContext } from '.';

export function PaperProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const res = await getPaperStatus();
      if (res.success && res.data) setEnabled(res.data.paper_mode);
    } catch {
      // ignore
    }
  }, []);

  const toggle = useCallback(async () => {
    if (loading) return;
    setLoading(true);
    try {
      if (enabled) {
        await disablePaperMode();
        setEnabled(false);
      } else {
        await enablePaperMode();
        setEnabled(true);
      }
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, [enabled, loading]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <PaperContext.Provider value={{ enabled, loading, toggle, refresh }}>
      {children}
    </PaperContext.Provider>
  );
}

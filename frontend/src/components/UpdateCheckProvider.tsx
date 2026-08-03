'use client';

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { updateService, type UpdateInfo } from '@/services/updateService';
import { UpdateDialog } from './UpdateDialog';

interface UpdateContextValue {
  isChecking: boolean;
  checkForUpdates: (showResult?: boolean) => Promise<void>;
}

const UpdateContext = createContext<UpdateContextValue | null>(null);

export function UpdateCheckProvider({ children }: { children: React.ReactNode }) {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  const checkForUpdates = useCallback(async (showResult = true) => {
    setIsChecking(true);
    setError(null);
    if (showResult) setOpen(true);

    try {
      const info = await updateService.checkForUpdates(showResult);
      setUpdateInfo(info);
      if (info.available && !showResult) {
        toast.info(`Empathy.AI ${info.version} está disponível`, {
          action: { label: 'Ver', onClick: () => setOpen(true) },
          duration: 12_000,
        });
      }
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(message);
      if (showResult) setOpen(true);
    } finally {
      setIsChecking(false);
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void checkForUpdates(false), 10_000);
    return () => window.clearTimeout(timer);
  }, [checkForUpdates]);

  const value = useMemo(() => ({ isChecking, checkForUpdates }), [isChecking, checkForUpdates]);

  return (
    <UpdateContext.Provider value={value}>
      {children}
      <UpdateDialog open={open} onOpenChange={setOpen} updateInfo={updateInfo} isChecking={isChecking} error={error} />
    </UpdateContext.Provider>
  );
}

export function useUpdates() {
  const context = useContext(UpdateContext);
  if (!context) throw new Error('useUpdates must be used inside UpdateCheckProvider');
  return context;
}

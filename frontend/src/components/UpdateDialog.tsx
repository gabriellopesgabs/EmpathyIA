'use client';

import { useState } from 'react';
import { AlertCircle, CheckCircle2, Download, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from './ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from './ui/dialog';
import { updateService, type UpdateInfo, type UpdateProgress } from '@/services/updateService';

interface UpdateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  updateInfo: UpdateInfo | null;
  isChecking: boolean;
  error: string | null;
}

export function UpdateDialog({ open, onOpenChange, updateInfo, isChecking, error }: UpdateDialogProps) {
  const [isInstalling, setIsInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const visibleError = installError ?? error;

  const setOpen = (nextOpen: boolean) => {
    if (!nextOpen && isInstalling) return;
    if (!nextOpen) {
      setProgress(null);
      setInstallError(null);
    }
    onOpenChange(nextOpen);
  };

  const install = async () => {
    setIsInstalling(true);
    setInstallError(null);
    try {
      await updateService.downloadAndInstall(setProgress);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setInstallError(message);
      setIsInstalling(false);
      toast.error(`Não foi possível atualizar: ${message}`);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        className="sm:max-w-[500px]"
        onEscapeKeyDown={(event) => isInstalling && event.preventDefault()}
        onInteractOutside={(event) => isInstalling && event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {(isChecking || isInstalling) && <Loader2 className="h-5 w-5 animate-spin" />}
            {visibleError && <AlertCircle className="h-5 w-5 text-red-600" />}
            {!isChecking && !isInstalling && !visibleError && updateInfo?.available && <Download className="h-5 w-5 text-blue-600" />}
            {!isChecking && !isInstalling && !visibleError && !updateInfo?.available && <CheckCircle2 className="h-5 w-5 text-emerald-600" />}
            Atualizações do Empathy.AI
          </DialogTitle>
          <DialogDescription>
            {isChecking && 'Consultando o canal oficial do EmpathyIA...'}
            {isInstalling && 'Baixando e verificando a atualização assinada...'}
            {!isChecking && !isInstalling && visibleError && 'Não foi possível consultar ou instalar a atualização.'}
            {!isChecking && !isInstalling && !visibleError && updateInfo?.available && `A versão ${updateInfo.version} está disponível.`}
            {!isChecking && !isInstalling && !visibleError && !updateInfo?.available && 'Você já está usando a versão mais recente.'}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {updateInfo?.available && !visibleError && !isInstalling && (
            <>
              <div className="grid grid-cols-2 gap-2 text-sm">
                <span className="text-muted-foreground">Versão instalada</span>
                <span className="font-medium text-right">{updateInfo.currentVersion}</span>
                <span className="text-muted-foreground">Nova versão</span>
                <span className="font-medium text-right">{updateInfo.version}</span>
              </div>
              {updateInfo.body && (
                <div className="max-h-48 overflow-y-auto rounded-lg bg-gray-50 p-3 text-sm whitespace-pre-wrap">
                  {updateInfo.body}
                </div>
              )}
            </>
          )}

          {isInstalling && progress && (
            <div className="space-y-2">
              <div className="h-3 overflow-hidden rounded-full bg-gray-200">
                <div className="h-full bg-blue-600 transition-all" style={{ width: `${progress.percentage}%` }} />
              </div>
              <p className="text-center text-sm text-muted-foreground">{progress.percentage}%</p>
            </div>
          )}

          {visibleError && <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-800">{visibleError}</div>}
        </div>

        <DialogFooter>
          {!isInstalling && <Button variant="outline" onClick={() => setOpen(false)}>Fechar</Button>}
          {updateInfo?.available && !visibleError && !isChecking && !isInstalling && (
            <Button onClick={install}><Download className="mr-2 h-4 w-4" />Baixar e instalar</Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

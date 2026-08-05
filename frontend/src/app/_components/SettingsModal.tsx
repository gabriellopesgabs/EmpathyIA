'use client';

import { useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { DeviceSelection } from '@/components/DeviceSelection';
import { LanguageSelection } from '@/components/LanguageSelection';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { useConfig } from '@/contexts/ConfigContext';
import { useRecordingState } from '@/contexts/RecordingStateContext';

type ModalType = 'modelSettings' | 'deviceSettings' | 'languageSettings' | 'modelSelector' | 'errorAlert' | 'chunkDropWarning';

interface SettingsModalsProps {
  modals: Record<ModalType, boolean>;
  messages: { errorAlert: string; chunkDropWarning: string; modelSelector: string };
  onClose: (name: ModalType) => void;
}

export function SettingsModals({ modals, messages, onClose }: SettingsModalsProps) {
  const router = useRouter();
  const { selectedDevices, setSelectedDevices, selectedLanguage, setSelectedLanguage, transcriptModelConfig } = useConfig();
  const { isRecording } = useRecordingState();

  useEffect(() => {
    if (!modals.modelSettings) return;
    onClose('modelSettings');
    router.push('/settings?tab=summaryModels');
  }, [modals.modelSettings, onClose, router]);

  useEffect(() => {
    if (modals.errorAlert) {
      toast.error('A gravação foi interrompida', { description: messages.errorAlert });
      onClose('errorAlert');
    }
  }, [messages.errorAlert, modals.errorAlert, onClose]);

  useEffect(() => {
    if (modals.chunkDropWarning) {
      toast.warning('A transcrição está mais lenta que o áudio', { description: messages.chunkDropWarning, duration: 8000 });
      onClose('chunkDropWarning');
    }
  }, [messages.chunkDropWarning, modals.chunkDropWarning, onClose]);

  const openTranscriptionSettings = () => {
    onClose('modelSelector');
    router.push('/settings?tab=transcriptionModels');
  };

  return (
    <>
      <Dialog open={modals.deviceSettings} onOpenChange={open => !open && onClose('deviceSettings')}>
        <DialogContent>
          <DialogHeader><DialogTitle>Dispositivos de áudio</DialogTitle><DialogDescription>Escolha as fontes usadas nesta gravação.</DialogDescription></DialogHeader>
          <DeviceSelection selectedDevices={selectedDevices} onDeviceChange={setSelectedDevices} disabled={isRecording} />
          <DialogFooter><Button onClick={() => onClose('deviceSettings')}>Concluído</Button></DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={modals.languageSettings} onOpenChange={open => !open && onClose('languageSettings')}>
        <DialogContent>
          <DialogHeader><DialogTitle>Idioma da transcrição</DialogTitle><DialogDescription>Automático funciona melhor quando a conversa usa principalmente um idioma.</DialogDescription></DialogHeader>
          <LanguageSelection selectedLanguage={selectedLanguage} onLanguageChange={setSelectedLanguage} disabled={isRecording} provider={transcriptModelConfig.provider} />
          <DialogFooter><Button onClick={() => onClose('languageSettings')}>Concluído</Button></DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={modals.modelSelector} onOpenChange={open => !open && onClose('modelSelector')}>
        <DialogContent>
          <DialogHeader><DialogTitle>Prepare a transcrição</DialogTitle><DialogDescription>{messages.modelSelector || 'Escolha ou baixe um modelo de reconhecimento de fala antes de gravar.'}</DialogDescription></DialogHeader>
          <DialogFooter><Button variant="ghost" onClick={() => onClose('modelSelector')}>Agora não</Button><Button onClick={openTranscriptionSettings}>Abrir Configurações</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

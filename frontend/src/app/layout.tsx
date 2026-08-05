'use client'

import './globals.css'
import { AppSidebar } from '@/components/AppShell/AppSidebar'
import { NotesColumn } from '@/components/AppShell/NotesColumn'
import { SidebarProvider } from '@/components/Sidebar/SidebarProvider'
import MainContent from '@/components/MainContent'
import AnalyticsProvider from '@/components/AnalyticsProvider'
import { Toaster, toast } from 'sonner'
import "sonner/dist/styles.css"
import { useState, useEffect, useCallback } from 'react'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { TooltipProvider } from '@/components/ui/tooltip'
import { RecordingStateProvider } from '@/contexts/RecordingStateContext'
import { OllamaDownloadProvider } from '@/contexts/OllamaDownloadContext'
import { TranscriptProvider } from '@/contexts/TranscriptContext'
import { ConfigProvider } from '@/contexts/ConfigContext'
import { OnboardingProvider } from '@/contexts/OnboardingContext'
import { OnboardingFlow } from '@/components/onboarding'
import { DownloadProgressToastProvider } from '@/components/shared/DownloadProgressToast'
import { UpdateCheckProvider } from '@/components/UpdateCheckProvider'
import { RecordingPostProcessingProvider } from '@/contexts/RecordingPostProcessingProvider'
import { ImportAudioDialog, ImportDropOverlay } from '@/components/ImportAudio'
import { ImportDialogProvider } from '@/contexts/ImportDialogContext'
import { isAudioExtension, getAudioFormatsDisplayList } from '@/constants/audioFormats'
import { GlobalCommandPalette } from '@/components/GlobalCommandPalette'
import { KnowledgeBootstrap } from '@/components/KnowledgeBootstrap'


// Module-level component — stable reference across RootLayout re-renders.
// Defined here (not inside RootLayout) so React never sees a new function type
// on re-render, which would cause unmount/remount and break initialization logic.
function ConditionalImportDialog({
  showImportDialog,
  handleImportDialogClose,
  importFilePath,
}: {
  showImportDialog: boolean;
  handleImportDialogClose: (open: boolean) => void;
  importFilePath: string | null;
}) {
  return (
    <ImportAudioDialog
      open={showImportDialog}
      onOpenChange={handleImportDialogClose}
      preselectedFile={importFilePath}
    />
  );
}

// export { metadata } from './metadata'

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const [showOnboarding, setShowOnboarding] = useState(false)

  // Import audio state
  const [showDropOverlay, setShowDropOverlay] = useState(false)
  const [showImportDialog, setShowImportDialog] = useState(false)
  const [importFilePath, setImportFilePath] = useState<string | null>(null)

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const applyTheme = () => document.documentElement.classList.toggle('dark', media.matches)
    applyTheme()
    media.addEventListener('change', applyTheme)
    return () => media.removeEventListener('change', applyTheme)
  }, [])

  useEffect(() => {
    // Check onboarding status first
    invoke<{ completed: boolean } | null>('get_onboarding_status')
      .then((status) => {
        const isComplete = status?.completed ?? false
        if (!isComplete) {
          console.log('[Layout] Onboarding not completed, showing onboarding flow')
          setShowOnboarding(true)
        } else {
          console.log('[Layout] Onboarding completed, showing main app')
        }
      })
      .catch((error) => {
        console.error('[Layout] Failed to check onboarding status:', error)
        // Default to showing onboarding if we can't check
        setShowOnboarding(true)
      })
  }, [])

  useEffect(() => {
    let unlistenFn: (() => void) | null = null;
    listen('request-recording-toggle', () => {
      console.log('[Layout] Received request-recording-toggle from tray');

      if (showOnboarding) {
        toast.error("Conclua a configuração primeiro", {
          description: "Finalize as etapas iniciais antes de começar uma gravação."
        });
      } else {
        console.log('[Layout] Forwarding to start-recording-from-sidebar');
        window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'));
      }
    }).then(fn => { unlistenFn = fn; });

    return () => {
      if (unlistenFn) {
        try { unlistenFn(); } catch {}
      }
    };
  }, [showOnboarding]);

  // Handle file drop for audio import
  const handleFileDrop = useCallback((paths: string[]) => {

    // Find the first audio file
    const audioFile = paths.find(p => {
      const ext = p.split('.').pop()?.toLowerCase();
      return !!ext && isAudioExtension(ext);
    });

    if (audioFile) {
      console.log('[Layout] Audio file dropped:', audioFile);
      setImportFilePath(audioFile);
      setShowImportDialog(true);
    } else if (paths.length > 0) {
      toast.error('Solte um arquivo de áudio', {
        description: `Formatos compatíveis: ${getAudioFormatsDisplayList()}`
      });
    }
  }, []);

  // Listen for drag-drop events
  useEffect(() => {
    if (showOnboarding) return; // Don't handle drops during onboarding

    const unlisteners: UnlistenFn[] = [];
    const cleanedUpRef = { current: false };

    const setupListeners = async () => {
      // Drag enter/over - show overlay only if beta feature is enabled
      const unlistenDragEnter = await listen('tauri://drag-enter', () => {
        setShowDropOverlay(true);
      });
      if (cleanedUpRef.current) {
        unlistenDragEnter();
        return;
      }
      unlisteners.push(unlistenDragEnter);

      // Drag leave - hide overlay
      const unlistenDragLeave = await listen('tauri://drag-leave', () => {
        setShowDropOverlay(false);
      });
      if (cleanedUpRef.current) {
        unlistenDragLeave();
        unlisteners.forEach(u => u());
        return;
      }
      unlisteners.push(unlistenDragLeave);

      // Drop - process files
      const unlistenDrop = await listen<{ paths: string[] }>('tauri://drag-drop', (event) => {
        setShowDropOverlay(false);
        handleFileDrop(event.payload.paths);
      });
      if (cleanedUpRef.current) {
        unlistenDrop();
        unlisteners.forEach(u => u());
        return;
      }
      unlisteners.push(unlistenDrop);
    };

    setupListeners();

    return () => {
      cleanedUpRef.current = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [showOnboarding, handleFileDrop]);

  // Handle import dialog close
  const handleImportDialogClose = useCallback((open: boolean) => {
    setShowImportDialog(open);
    if (!open) {
      setImportFilePath(null);
    }
  }, []);

  // Handler for ImportDialogProvider - opens import dialog from any child component
  const handleOpenImportDialog = useCallback((filePath?: string | null) => {
    setImportFilePath(filePath ?? null);
    setShowImportDialog(true);
  }, []);

  const handleOnboardingComplete = () => {
    console.log('[Layout] Onboarding completed, reloading app')
    setShowOnboarding(false)
    // Optionally reload the window to ensure all state is fresh
    window.location.reload()
  }

  return (
    <html lang="pt-BR" suppressHydrationWarning>
      <body className="font-sans antialiased">
        <AnalyticsProvider>
          <RecordingStateProvider>
            <TranscriptProvider>
              <ConfigProvider>
                <OllamaDownloadProvider>
                  <OnboardingProvider>
                    <UpdateCheckProvider>
                      <SidebarProvider>
                        <TooltipProvider>
                          <RecordingPostProcessingProvider>
                            <ImportDialogProvider onOpen={handleOpenImportDialog}>
                              {/* Download progress toast provider - listens for background downloads */}
                              <DownloadProgressToastProvider />

                              {/* Show onboarding or main app */}
                              {showOnboarding ? (
                                <OnboardingFlow onComplete={handleOnboardingComplete} />
                              ) : (
                                <div className="app-shell">
                                  <GlobalCommandPalette />
                                  <KnowledgeBootstrap />
                                  <AppSidebar />
                                  <NotesColumn />
                                  <MainContent>{children}</MainContent>
                                </div>
                              )}
                              {/* Import audio overlay and dialog */}
                              <ImportDropOverlay visible={showDropOverlay} />
                              <ConditionalImportDialog
                                showImportDialog={showImportDialog}
                                handleImportDialogClose={handleImportDialogClose}
                                importFilePath={importFilePath}
                              />
                            </ImportDialogProvider>
                          </RecordingPostProcessingProvider>
                        </TooltipProvider>
                      </SidebarProvider>
                    </UpdateCheckProvider>
                  </OnboardingProvider>

                </OllamaDownloadProvider>
              </ConfigProvider>
            </TranscriptProvider>
          </RecordingStateProvider>
        </AnalyticsProvider>

        <Toaster position="bottom-center" richColors closeButton />
      </body>
    </html>
  )
}

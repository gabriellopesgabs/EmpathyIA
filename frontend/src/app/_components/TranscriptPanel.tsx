import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import { PermissionWarning } from '@/components/PermissionWarning';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import { Columns2, Copy, FileText, GlobeIcon, Network } from 'lucide-react';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useConfig } from '@/contexts/ConfigContext';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { usePermissionCheck } from '@/hooks/usePermissionCheck';
import { ModalType } from '@/hooks/useModalState';
import { useIsLinux } from '@/hooks/usePlatform';
import { useEffect, useMemo, useState } from 'react';
import { KnowledgeGraphView } from '@/components/KnowledgeGraph';
import { buildLiveKnowledgeGraph } from '@/lib/knowledgeGraph';

type TranscriptViewMode = 'transcript' | 'split' | 'graph';

/**
 * TranscriptPanel Component
 *
 * Displays transcript content with controls for copying and language settings.
 * Uses TranscriptContext, ConfigContext, and RecordingStateContext internally.
 */

interface TranscriptPanelProps {
  // indicates stop-processing state for transcripts; derived from backend statuses.
  isProcessingStop: boolean;
  isStopping: boolean;
  showModal: (name: ModalType, message?: string) => void;
}

export function TranscriptPanel({
  isProcessingStop,
  isStopping,
  showModal
}: TranscriptPanelProps) {
  // Contexts
  const { transcripts, transcriptContainerRef, copyTranscript, meetingTitle } = useTranscripts();
  const { transcriptModelConfig } = useConfig();
  const { isRecording, isPaused } = useRecordingState();
  const { checkPermissions, isChecking, hasSystemAudio, hasMicrophone } = usePermissionCheck();
  const isLinux = useIsLinux();
  const [viewMode, setViewMode] = useState<TranscriptViewMode>('transcript');

  // Convert transcripts to segments for virtualized view
  const segments = useMemo(() =>
    transcripts.map(t => ({
      id: t.id,
      timestamp: t.audio_start_time ?? 0,
      endTime: t.audio_end_time,
      text: t.text,
      confidence: t.confidence,
    })),
    [transcripts]
  );
  const liveGraph = useMemo(() => buildLiveKnowledgeGraph(transcripts, meetingTitle), [meetingTitle, transcripts]);
  const partialCount = transcripts.filter(transcript => transcript.is_partial).length;
  const graphStatusLabel = isRecording
    ? (isPaused ? 'Pausado' : transcripts.length > 0 ? 'Ao vivo' : 'Ouvindo')
    : isProcessingStop ? 'Processando' : undefined;

  useEffect(() => {
    const saved = localStorage.getItem('empathy_live_transcript_view');
    if (saved === 'transcript' || saved === 'split' || saved === 'graph') setViewMode(saved);
  }, []);

  const changeView = (mode: TranscriptViewMode) => {
    setViewMode(mode);
    localStorage.setItem('empathy_live_transcript_view', mode);
  };

  const transcriptContent = (
    <div className="flex min-h-full justify-center pb-20">
      <div className={viewMode === 'split' ? 'w-full max-w-[750px] px-2 sm:px-4' : 'w-full max-w-4xl px-3 sm:px-6'}>
        <VirtualizedTranscriptView
          segments={segments}
          isRecording={isRecording}
          isPaused={isPaused}
          isProcessing={isProcessingStop}
          isStopping={isStopping}
          enableStreaming={isRecording}
          showConfidence={true}
        />
      </div>
    </div>
  );

  return (
    <div className="w-full border-r border-gray-200 bg-white flex flex-col overflow-hidden">
      {/* Title area - Sticky header */}
      <div className="sticky top-0 z-10 bg-white p-4 border-gray-200">
        <div className="flex flex-col space-y-3">
          <div className="flex  flex-col space-y-2">
            <div className="flex justify-center  items-center space-x-2">
              <ButtonGroup>
                {transcripts?.length > 0 && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={copyTranscript}
                    title="Copy Transcript"
                  >
                    <Copy />
                    <span className='hidden md:inline'>
                      Copy
                    </span>
                  </Button>
                )}
                {transcriptModelConfig.provider === "localWhisper" &&
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => showModal('languageSettings')}
                    title="Language"
                  >
                    <GlobeIcon />
                    <span className='hidden md:inline'>
                      Language
                    </span>
                  </Button>
                }
              </ButtonGroup>
              <ButtonGroup aria-label="Visualização da reunião ao vivo">
                <Button variant={viewMode === 'transcript' ? 'secondary' : 'outline'} size="sm" onClick={() => changeView('transcript')} title="Transcrição">
                  <FileText /><span className="hidden lg:inline">Transcrição</span>
                </Button>
                <Button variant={viewMode === 'split' ? 'secondary' : 'outline'} size="sm" onClick={() => changeView('split')} title="Transcrição e grafo">
                  <Columns2 /><span className="hidden lg:inline">Dividido</span>
                </Button>
                <Button variant={viewMode === 'graph' ? 'secondary' : 'outline'} size="sm" onClick={() => changeView('graph')} title="Grafo ao vivo">
                  <Network /><span className="hidden lg:inline">Grafo</span>
                </Button>
              </ButtonGroup>
            </div>
          </div>
        </div>
      </div>

      {/* Permission Warning - Not needed on Linux */}
      {!isRecording && !isChecking && !isLinux && (
        <div className="flex justify-center px-4 pt-4">
          <PermissionWarning
            hasMicrophone={hasMicrophone}
            hasSystemAudio={hasSystemAudio}
            onRecheck={checkPermissions}
            isRechecking={isChecking}
          />
        </div>
      )}

      <div className={`min-h-0 flex-1 ${viewMode === 'split' ? 'grid grid-rows-2 lg:grid-cols-2 lg:grid-rows-1' : ''}`}>
        {viewMode !== 'graph' && (
          <div ref={transcriptContainerRef} className="h-full overflow-y-auto">
            {transcriptContent}
          </div>
        )}
        {viewMode !== 'transcript' && (
          <div className="h-full overflow-y-auto border-l bg-slate-50 p-3 dark:bg-gray-950">
            <KnowledgeGraphView
              graph={liveGraph}
              title="Temas da conversa"
              subtitle={transcripts.length === 0
                ? 'Os temas aparecerão conforme a fala for transcrita.'
                : `${transcripts.length} trecho${transcripts.length === 1 ? '' : 's'} recebido${transcripts.length === 1 ? '' : 's'}${partialCount > 0 ? ` · ${partialCount} em processamento` : ''}.`}
              live={isRecording && !isPaused}
              statusLabel={graphStatusLabel}
            />
          </div>
        )}
      </div>
    </div>
  );
}

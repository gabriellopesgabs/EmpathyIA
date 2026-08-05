"use client";

import { Transcript, TranscriptSegmentData } from '@/types';
import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import { TranscriptButtonGroup } from './TranscriptButtonGroup';
import { useMemo } from 'react';

interface TranscriptPanelProps {
  transcripts: Transcript[];
  customPrompt: string;
  onPromptChange: (value: string) => void;
  onCopyTranscript: () => void;
  onOpenMeetingFolder: () => Promise<void>;
  isRecording: boolean;
  disableAutoScroll?: boolean;

  // Optional pagination props (when using virtualization)
  usePagination?: boolean;
  segments?: TranscriptSegmentData[];
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;

  // Retranscription props
  meetingId?: string;
  meetingFolderPath?: string | null;
  onRefetchTranscripts?: () => Promise<void>;
}

export function TranscriptPanel({
  transcripts,
  customPrompt,
  onPromptChange,
  onCopyTranscript,
  onOpenMeetingFolder,
  isRecording,
  disableAutoScroll = false,
  usePagination = false,
  segments,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
  meetingId,
  meetingFolderPath,
  onRefetchTranscripts,
}: TranscriptPanelProps) {
  // Convert transcripts to segments if pagination is not used but we want virtualization
  const convertedSegments = useMemo(() => {
    if (usePagination && segments) {
      return segments;
    }
    // Convert transcripts to segments for virtualization
    return transcripts.map(t => ({
      id: t.id,
      timestamp: t.audio_start_time ?? 0,
      endTime: t.audio_end_time,
      text: t.text,
      confidence: t.confidence,
    }));
  }, [transcripts, usePagination, segments]);

  return (
    <div className="flex h-full w-full min-w-0 flex-col bg-card">
      {/* Title area */}
      <div className="border-b border-border p-4">
        <TranscriptButtonGroup
          transcriptCount={usePagination ? (totalCount ?? convertedSegments.length) : (transcripts?.length || 0)}
          onCopyTranscript={onCopyTranscript}
          onOpenMeetingFolder={onOpenMeetingFolder}
          meetingId={meetingId}
          meetingFolderPath={meetingFolderPath}
          onRefetchTranscripts={onRefetchTranscripts}
        />
      </div>

      {/* Transcript content - use virtualized view for better performance */}
      <div className="flex-1 overflow-hidden pb-4">
        <VirtualizedTranscriptView
          segments={convertedSegments}
          isRecording={isRecording}
          isPaused={false}
          isProcessing={false}
          isStopping={false}
          enableStreaming={false}
          showConfidence={true}
          disableAutoScroll={disableAutoScroll}
          hasMore={hasMore}
          isLoadingMore={isLoadingMore}
          totalCount={totalCount}
          loadedCount={loadedCount}
          onLoadMore={onLoadMore}
        />
      </div>

      {/* Custom prompt input at bottom of transcript section */}
      {!isRecording && convertedSegments.length > 0 && (
        <div className="flex flex-col gap-2 border-t border-border bg-muted/40 p-3">
          <div className="flex flex-wrap gap-1.5 items-center">
            <span className="mr-1 text-xs font-semibold text-muted-foreground">Perspectivas:</span>
            {[
              { name: '🛠️ Software Architect', prompt: 'Analyze with focus on software architecture, technical decisions, trade-offs, and technical debt.' },
              { name: '🏃 Agile Coach', prompt: 'Analyze team dynamics, agile processes, blockers, and improvements.' },
              { name: '📋 Project Manager', prompt: 'Analyze with focus on milestones, action items, owner assignments, and deadlines.' },
              { name: '🎨 UX Designer', prompt: 'Focus on user experience, design decisions, usability, and feedback.' },
            ].map((skill) => {
              const isSelected = customPrompt.includes(skill.prompt);
              return (
                <button
                  key={skill.name}
                  onClick={() => {
                    if (isSelected) {
                      const updated = customPrompt.replace(skill.prompt, '').replace(/\n\s*\n/g, '\n').trim();
                      onPromptChange(updated);
                    } else {
                      const updated = customPrompt ? `${customPrompt}\n\n${skill.prompt}` : skill.prompt;
                      onPromptChange(updated.trim());
                    }
                  }}
                  className={`px-2 py-1 text-xs font-medium rounded-full border transition-all duration-200 ${
                    isSelected
                      ? 'border-primary bg-primary text-primary-foreground shadow-sm'
                      : 'border-border bg-card text-foreground hover:border-foreground/20 hover:bg-muted'
                  }`}
                >
                  {skill.name}
                </button>
              );
            })}
          </div>
          <textarea
            placeholder="Adicione contexto para o resumo: pessoas, objetivo ou informações importantes…"
            className="min-h-[80px] w-full resize-y rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground shadow-sm placeholder:text-muted-foreground focus:border-ring focus:outline-none focus:ring-1 focus:ring-ring"
            value={customPrompt}
            onChange={(e) => onPromptChange(e.target.value)}
          />
        </div>
      )}
    </div>
  );
}

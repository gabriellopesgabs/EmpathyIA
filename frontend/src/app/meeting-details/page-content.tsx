'use client';

import { useEffect, useMemo, useState } from 'react';
import { motion } from 'framer-motion';
import { FileText, Info, Network, ScrollText } from 'lucide-react';
import Analytics from '@/lib/analytics';
import { buildLiveKnowledgeGraph, buildMarkdownKnowledgeGraph, mergeMeetingKnowledgeGraphs } from '@/lib/knowledgeGraph';
import { TranscriptPanel } from '@/components/MeetingDetails/TranscriptPanel';
import { MeetingKnowledgeGraph } from '@/components/MeetingDetails/MeetingKnowledgeGraph';
import { MeetingPropertiesEditor } from '@/components/MeetingDetails/MeetingPropertiesEditor';
import { RelatedMeetings } from '@/components/MeetingDetails/RelatedMeetings';
import { AugmentedNoteEditor } from '@/components/Notes/AugmentedNoteEditor';
import { SkillPanel } from '@/components/Skills/SkillPanel';
import { Button } from '@/components/ui/button';
import { buildSkillResultBlock, type TextSelection } from '@/lib/skillBlocks';
import { noteService, type NoteDocument } from '@/services/noteService';
import { useMeetingData } from '@/hooks/meeting-details/useMeetingData';
import { useCopyOperations } from '@/hooks/meeting-details/useCopyOperations';
import { useMeetingOperations } from '@/hooks/meeting-details/useMeetingOperations';
import type { Summary } from '@/types';
import type { SkillRunResult } from '@/types/skills';
import { toast } from 'sonner';

export default function PageContent({ meeting, summaryData, onMeetingUpdated, onRefetchTranscripts, segments, hasMore, isLoadingMore, totalCount, loadedCount, onLoadMore }: {
  meeting: any; summaryData: Summary | null; shouldAutoGenerate?: boolean; onAutoGenerateComplete?: () => void;
  onMeetingUpdated?: () => Promise<void>; onRefetchTranscripts?: () => Promise<void>; segments?: any[];
  hasMore?: boolean; isLoadingMore?: boolean; totalCount?: number; loadedCount?: number; onLoadMore?: () => void;
}) {
  const [activeView, setActiveView] = useState<'note' | 'transcript' | 'graph'>('note');
  const [showInspector, setShowInspector] = useState(false);
  const [noteMarkdown, setNoteMarkdown] = useState('');
  const [transcriptSkillOpen, setTranscriptSkillOpen] = useState(false);
  const [transcriptSkillNote, setTranscriptSkillNote] = useState<NoteDocument | null>(null);
  const [transcriptSelection, setTranscriptSelection] = useState<TextSelection | null>(null);
  const meetingData = useMeetingData({ meeting, summaryData, onMeetingUpdated });
  const transcriptMarkdown = useMemo(() => meetingData.transcripts.map((item: { timestamp: string; text: string }) => `[${item.timestamp}] ${item.text}`).join('\n\n'), [meetingData.transcripts]);
  const fallbackGraph = useMemo(() => mergeMeetingKnowledgeGraphs(
    buildLiveKnowledgeGraph(meetingData.transcripts, meetingData.meetingTitle),
    buildMarkdownKnowledgeGraph(noteMarkdown, meetingData.meetingTitle),
  ), [meetingData.meetingTitle, meetingData.transcripts, noteMarkdown]);
  const copyOperations = useCopyOperations({ meeting, transcripts: meetingData.transcripts, meetingTitle: meetingData.meetingTitle, aiSummary: meetingData.aiSummary, blockNoteSummaryRef: meetingData.blockNoteSummaryRef });
  const meetingOperations = useMeetingOperations({ meeting });
  useEffect(() => { Analytics.trackPageView('meeting_details'); }, []);

  const openTranscriptSkills = async () => {
    const selectedText = window.getSelection()?.toString().trim() || '';
    try {
      const document = await noteService.get(meeting.id);
      setTranscriptSkillNote(document);
      setNoteMarkdown(document.content);
      setTranscriptSelection(selectedText ? { start: 0, end: selectedText.length, text: selectedText } : null);
      setTranscriptSkillOpen(true);
    } catch (error) {
      toast.error('Não foi possível preparar a Nota', { description: String(error) });
    }
  };

  const incorporateTranscriptSkill = async (result: SkillRunResult, resultTitle: string, markdown: string) => {
    if (!transcriptSkillNote) throw new Error('A Nota não está disponível.');
    const block = buildSkillResultBlock(result, resultTitle, markdown);
    const nextContent = [transcriptSkillNote.content.trimEnd(), block].filter(Boolean).join('\n\n');
    try {
      const saved = await noteService.save(transcriptSkillNote.id, meetingData.meetingTitle, nextContent, transcriptSkillNote.content_hash);
      setTranscriptSkillNote(saved);
      setNoteMarkdown(saved.content);
      window.dispatchEvent(new CustomEvent('notes-changed'));
      window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      toast.success('Resultado incorporado à Nota', { description: 'A transcrição original foi preservada.' });
      setActiveView('note');
    } catch (error) {
      toast.error('A Nota não foi alterada', { description: String(error) });
      throw error;
    }
  };

  return <motion.div initial={{ opacity: 1 }} animate={{ opacity: 1 }} className="document-surface">
    <header className="document-toolbar">
      <input className="min-w-0 max-w-sm flex-1 truncate border-0 bg-transparent text-sm font-semibold outline-none" value={meetingData.meetingTitle} onChange={event => meetingData.handleTitleChange(event.target.value)} onBlur={() => meetingData.isTitleDirty && void meetingData.saveAllChanges()} aria-label="Título da nota" />
      <div className="segmented-control" role="tablist" aria-label="Conteúdo da nota gravada">
        <button type="button" role="tab" data-active={activeView === 'note'} aria-selected={activeView === 'note'} onClick={() => setActiveView('note')}><FileText />Nota</button>
        <button type="button" role="tab" data-active={activeView === 'transcript'} aria-selected={activeView === 'transcript'} onClick={() => setActiveView('transcript')}><ScrollText />Transcrição</button>
        <button type="button" role="tab" data-active={activeView === 'graph'} aria-selected={activeView === 'graph'} onClick={() => setActiveView('graph')}><Network />Grafo</button>
      </div>
      <Button variant={showInspector ? 'secondary' : 'ghost'} size="icon" onClick={() => setShowInspector(value => !value)} aria-label="Mostrar informações da nota"><Info /></Button>
    </header>
    <div className="flex min-h-0 flex-1 overflow-hidden">
      <div className="min-w-0 flex-1">
        {activeView === 'note' && <AugmentedNoteEditor id={meeting.id} transcript={transcriptMarkdown} compact externalTitle={meetingData.meetingTitle} onContentChanged={setNoteMarkdown} />}
        {activeView === 'transcript' && <TranscriptPanel transcripts={meetingData.transcripts} customPrompt="" onPromptChange={() => undefined} onCopyTranscript={copyOperations.handleCopyTranscript} onOpenMeetingFolder={meetingOperations.handleOpenMeetingFolder} onComposeNote={() => void openTranscriptSkills()} isRecording={false} disableAutoScroll usePagination segments={segments} hasMore={hasMore} isLoadingMore={isLoadingMore} totalCount={totalCount} loadedCount={loadedCount} onLoadMore={onLoadMore} meetingId={meeting.id} meetingFolderPath={meeting.folder_path} onRefetchTranscripts={onRefetchTranscripts} />}
        {activeView === 'graph' && <div className="document-scroll p-4"><MeetingKnowledgeGraph meetingId={meeting.id} fallbackGraph={fallbackGraph} /></div>}
      </div>
      {activeView === 'transcript' && transcriptSkillNote && <SkillPanel open={transcriptSkillOpen} note={{ ...transcriptSkillNote, title: meetingData.meetingTitle }} content={transcriptSkillNote.content} selection={transcriptSelection} transcript={transcriptMarkdown} sourceMode="transcript" onClose={() => setTranscriptSkillOpen(false)} onAccept={incorporateTranscriptSkill} />}
      {showInspector && <aside className="note-inspector"><div><p className="eyebrow">Informações</p><h2>{meetingData.meetingTitle}</h2></div><MeetingPropertiesEditor meetingId={meeting.id} /><RelatedMeetings meetingId={meeting.id} /></aside>}
    </div>
  </motion.div>;
}

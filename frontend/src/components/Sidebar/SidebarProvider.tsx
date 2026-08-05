'use client';

import React, { createContext, useContext, useState, useEffect } from 'react';
import { usePathname, useRouter } from 'next/navigation';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { noteService } from '@/services/noteService';
import { toast } from 'sonner';


export interface CurrentMeeting {
  id: string;
  title: string;
  recorded?: boolean;
  written?: boolean;
  archived?: boolean;
}

interface SidebarContextType {
  currentMeeting: CurrentMeeting | null;
  setCurrentMeeting: (meeting: CurrentMeeting | null) => void;
  isCollapsed: boolean;
  toggleCollapse: () => void;
  meetings: CurrentMeeting[];
  setMeetings: (meetings: CurrentMeeting[]) => void;
  isMeetingActive: boolean;
  setIsMeetingActive: (active: boolean) => void;
  handleRecordingToggle: () => void;
  serverAddress: string;
  // Summary polling management
  activeSummaryPolls: Map<string, NodeJS.Timeout>;
  startSummaryPolling: (meetingId: string, processId: string, onUpdate: (result: any) => void) => void;
  stopSummaryPolling: (meetingId: string) => void;
  // Refetch meetings from backend
  refetchMeetings: () => Promise<void>;

}

const SidebarContext = createContext<SidebarContextType | null>(null);

export const useSidebar = () => {
  const context = useContext(SidebarContext);
  if (!context) {
    throw new Error('useSidebar must be used within a SidebarProvider');
  }
  return context;
};

export function SidebarProvider({ children }: { children: React.ReactNode }) {
  const [currentMeeting, setCurrentMeeting] = useState<CurrentMeeting | null>({ id: 'intro-call', title: 'Nova conversa' });
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [meetings, setMeetings] = useState<CurrentMeeting[]>([]);
  const [isMeetingActive, setIsMeetingActive] = useState(false);
  const serverAddress = 'native';
  const [activeSummaryPolls, setActiveSummaryPolls] = useState<Map<string, NodeJS.Timeout>>(new Map());

  // Use recording state from RecordingStateContext (single source of truth)
  const { isRecording } = useRecordingState();

  const pathname = usePathname();
  const router = useRouter();

  // Extract fetchMeetings as a reusable function
  const fetchMeetings = React.useCallback(async () => {
      try {
        const meetings = await invoke('api_get_meetings') as Array<{ id: string, title: string, recorded: boolean, written: boolean, archived: boolean }>;
        const transformedMeetings = meetings.map((meeting: any) => ({
          id: meeting.id,
          title: meeting.title,
          recorded: meeting.recorded,
          written: meeting.written,
          archived: meeting.archived,
        }));
        setMeetings(transformedMeetings);
        Analytics.trackBackendConnection(true);
      } catch (error) {
        console.error('Error fetching meetings:', error);
        setMeetings([]);
        Analytics.trackBackendConnection(false, error instanceof Error ? error.message : 'Unknown error');
      }
  }, []);

  useEffect(() => {
    fetchMeetings();
  }, [serverAddress, fetchMeetings]);

  useEffect(() => {
    void noteService.migrateLegacyNotes()
      .then(count => {
        if (count === 0) return;
        void fetchMeetings();
        toast.success(`${count} ${count === 1 ? 'nota antiga foi migrada' : 'notas antigas foram migradas'} para Markdown`);
      })
      .catch(error => toast.error('Não foi possível migrar as notas antigas', { description: String(error) }));
  }, [fetchMeetings]);

  useEffect(() => {
    const refresh = () => { void fetchMeetings(); };
    window.addEventListener('notes-changed', refresh);
    return () => window.removeEventListener('notes-changed', refresh);
  }, [fetchMeetings]);

  useEffect(() => {
    const markWritten = (event: Event) => {
      const meetingId = (event as CustomEvent<{ meetingId: string }>).detail?.meetingId;
      if (!meetingId) return;
      setMeetings(current => current.map(meeting => (
        meeting.id === meetingId ? { ...meeting, recorded: true, written: true } : meeting
      )));
    };
    window.addEventListener('meeting-note-written', markWritten);
    return () => window.removeEventListener('meeting-note-written', markWritten);
  }, []);


  const toggleCollapse = React.useCallback(() => setIsCollapsed(value => !value), []);

  // Update current meeting when on home page
  useEffect(() => {
    if (pathname === '/') {
      setCurrentMeeting({ id: 'intro-call', title: 'Nova conversa' });
    }
  }, [pathname]);

  // Function to handle recording toggle from sidebar
  const handleRecordingToggle = () => {
    if (isRecording) {
      const stopRecording = (window as Window & { handleRecordingStop?: (callApi?: boolean) => void }).handleRecordingStop;
      stopRecording?.(true);
      Analytics.trackButtonClick('stop_recording', 'sidebar');
      return;
    }

    if (pathname === '/') {
      window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'));
    } else {
      sessionStorage.setItem('autoStartRecording', 'true');
      router.push('/');
    }
    Analytics.trackButtonClick('start_recording', 'sidebar');
  };

  // Summary polling management
  const startSummaryPolling = React.useCallback((
    meetingId: string,
    processId: string,
    onUpdate: (result: any) => void
  ) => {
    // Stop existing poll for this meeting if any
    if (activeSummaryPolls.has(meetingId)) {
      clearInterval(activeSummaryPolls.get(meetingId)!);
    }

    console.log(`📊 Starting polling for meeting ${meetingId}, process ${processId}`);

    let pollCount = 0;
    const MAX_POLLS = 200; // ~16.5 minutes at 5-second intervals (slightly longer than backend's 15-min timeout to avoid race conditions)

    const pollInterval = setInterval(async () => {
      pollCount++;

      // Timeout safety: Stop after 10 minutes
      if (pollCount >= MAX_POLLS) {
        console.warn(`⏱️ Polling timeout for ${meetingId} after ${MAX_POLLS} iterations`);
        clearInterval(pollInterval);
        setActiveSummaryPolls(prev => {
          const next = new Map(prev);
          next.delete(meetingId);
          return next;
        });
        onUpdate({
          status: 'error',
          error: 'Summary generation timed out after 15 minutes. Please try again or check your model configuration.'
        });
        return;
      }
      try {
        const result = await invoke('api_get_summary', {
          meetingId: meetingId,
        }) as any;

        console.log(`📊 Polling update for ${meetingId}:`, result.status);

        // Call the update callback with result
        onUpdate(result);

        // Stop polling if completed, error, failed, cancelled, or idle (after initial processing)
        if (result.status === 'completed' || result.status === 'error' || result.status === 'failed' || result.status === 'cancelled') {
          console.log(`Polling completed for ${meetingId}, status: ${result.status}`);
          clearInterval(pollInterval);
          setActiveSummaryPolls(prev => {
            const next = new Map(prev);
            next.delete(meetingId);
            return next;
          });
        } else if (result.status === 'idle' && pollCount > 1) {
          // If we get 'idle' after polling started, process completed/disappeared
          console.log(`Process completed or not found for ${meetingId}, stopping poll`);
          clearInterval(pollInterval);
          setActiveSummaryPolls(prev => {
            const next = new Map(prev);
            next.delete(meetingId);
            return next;
          });
        }
      } catch (error) {
        console.error(`Polling error for ${meetingId}:`, error);
        // Report error to callback
        onUpdate({
          status: 'error',
          error: error instanceof Error ? error.message : 'Unknown error'
        });
        clearInterval(pollInterval);
        setActiveSummaryPolls(prev => {
          const next = new Map(prev);
          next.delete(meetingId);
          return next;
        });
      }
    }, 5000); // Poll every 5 seconds

    setActiveSummaryPolls(prev => new Map(prev).set(meetingId, pollInterval));
  }, [activeSummaryPolls]);

  const stopSummaryPolling = React.useCallback((meetingId: string) => {
    const pollInterval = activeSummaryPolls.get(meetingId);
    if (pollInterval) {
      console.log(`⏹️ Stopping polling for meeting ${meetingId}`);
      clearInterval(pollInterval);
      setActiveSummaryPolls(prev => {
        const next = new Map(prev);
        next.delete(meetingId);
        return next;
      });
    }
  }, [activeSummaryPolls]);

  // Cleanup all polling intervals on unmount
  useEffect(() => {
    return () => {
      console.log('🧹 Cleaning up all summary polling intervals');
      activeSummaryPolls.forEach(interval => clearInterval(interval));
    };
  }, [activeSummaryPolls]);



  return (
    <SidebarContext.Provider value={{
      currentMeeting,
      setCurrentMeeting,
      isCollapsed,
      toggleCollapse,
      meetings,
      setMeetings,
      isMeetingActive,
      setIsMeetingActive,
      handleRecordingToggle,
      serverAddress,
      activeSummaryPolls,
      startSummaryPolling,
      stopSummaryPolling,
      refetchMeetings: fetchMeetings,

    }}>
      {children}
    </SidebarContext.Provider>
  );
}

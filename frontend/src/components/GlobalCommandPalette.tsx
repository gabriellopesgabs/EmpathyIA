'use client';

import { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { BookOpen, FilePlus2, Home, Mic, Search, Settings } from 'lucide-react';
import { noteService } from '@/services/noteService';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import {
  CommandDialog, CommandEmpty, CommandGroup, CommandInput,
  CommandItem, CommandList, CommandShortcut,
} from '@/components/ui/command';

type SearchResult = { path: string; meeting_id?: string; kind: string; title: string; snippet: string };

export function GlobalCommandPalette() {
  const router = useRouter();
  const { isRecording } = useRecordingState();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);

  const go = useCallback((path: string) => {
    setOpen(false);
    setQuery('');
    router.push(path);
  }, [router]);

  const createNote = useCallback(async () => {
    const note = await noteService.create();
    window.dispatchEvent(new CustomEvent('notes-changed'));
    go(`/notes?id=${note.id}`);
  }, [go]);

  const toggleRecording = useCallback(() => {
    setOpen(false);
    if (isRecording) {
      (window as Window & { handleRecordingStop?: (callApi?: boolean) => void }).handleRecordingStop?.(true);
      return;
    }
    go('/');
    window.setTimeout(() => window.dispatchEvent(new CustomEvent('start-recording-from-sidebar')), 100);
  }, [go, isRecording]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setOpen(value => !value);
      }
      if ((event.metaKey || event.ctrlKey) && !event.shiftKey && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        void createNote();
      }
      if ((event.metaKey || event.ctrlKey) && event.key === ',') {
        event.preventDefault();
        go('/settings');
      }
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'r') {
        event.preventDefault();
        toggleRecording();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [createNote, go, toggleRecording]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<string>('app-menu-command', event => {
      switch (event.payload) {
        case 'new_note': void createNote(); break;
        case 'record': toggleRecording(); break;
        case 'import_audio': window.dispatchEvent(new CustomEvent('open-import-dialog')); break;
        case 'search': setOpen(true); break;
        case 'knowledge': go('/knowledge'); break;
        case 'settings': go('/settings'); break;
        case 'toggle_sidebar': window.dispatchEvent(new CustomEvent('toggle-app-sidebar')); break;
      }
    }).then(fn => { unlisten = fn; });
    return () => unlisten?.();
  }, [createNote, go, toggleRecording]);

  useEffect(() => {
    const timer = setTimeout(async () => {
      if (!query.trim()) return setResults([]);
      try {
        setResults(await invoke<SearchResult[]>('api_search_knowledge', { query }));
      } catch {
        setResults([]);
      }
    }, 200);
    return () => clearTimeout(timer);
  }, [query]);

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput value={query} onValueChange={setQuery} placeholder="Buscar reuniões ou executar um comando…" />
      <CommandList>
        <CommandEmpty>Nenhum resultado.</CommandEmpty>
        <CommandGroup heading="Ações">
          <CommandItem onSelect={() => void createNote()}><FilePlus2 /> Nova nota<CommandShortcut>⌘N</CommandShortcut></CommandItem>
          <CommandItem onSelect={() => go('/')}><Home /> Início</CommandItem>
          <CommandItem onSelect={toggleRecording}><Mic /> {isRecording ? 'Encerrar gravação' : 'Nova gravação'}</CommandItem>
          <CommandItem onSelect={() => go('/knowledge')}><BookOpen /> Conhecimento</CommandItem>
          <CommandItem onSelect={() => go('/settings')}><Settings /> Configurações</CommandItem>
        </CommandGroup>
        {results.length > 0 && (
          <CommandGroup heading="Conhecimento">
            {results.slice(0, 12).map(result => (
              <CommandItem key={result.path} value={`${result.title} ${result.path}`}
                onSelect={() => result.meeting_id ? go(`/meeting-details?id=${result.meeting_id}`) : go('/knowledge')}>
                <Search /> <span className="truncate">{result.title}</span><CommandShortcut>{result.kind}</CommandShortcut>
              </CommandItem>
            ))}
          </CommandGroup>
        )}
      </CommandList>
    </CommandDialog>
  );
}

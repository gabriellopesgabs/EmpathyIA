'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { BookOpen, Home, Mic, Search, Settings } from 'lucide-react';
import {
  CommandDialog, CommandEmpty, CommandGroup, CommandInput,
  CommandItem, CommandList, CommandShortcut,
} from '@/components/ui/command';

type SearchResult = { path: string; meeting_id?: string; kind: string; title: string; snippet: string };

export function GlobalCommandPalette() {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setOpen(value => !value);
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

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

  const go = (path: string) => {
    setOpen(false);
    setQuery('');
    router.push(path);
  };

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput value={query} onValueChange={setQuery} placeholder="Buscar reuniões ou executar um comando…" />
      <CommandList>
        <CommandEmpty>Nenhum resultado.</CommandEmpty>
        <CommandGroup heading="Ações">
          <CommandItem onSelect={() => go('/')}><Home /> Início</CommandItem>
          <CommandItem onSelect={() => { setOpen(false); window.dispatchEvent(new CustomEvent('start-recording-from-sidebar')); }}><Mic /> Nova gravação</CommandItem>
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

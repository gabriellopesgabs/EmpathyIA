'use client';

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Calendar, FileText, FolderOpen, Loader2, Network, Save } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { KnowledgeGraphView } from '@/components/KnowledgeGraph';
import { buildMarkdownKnowledgeGraph } from '@/lib/knowledgeGraph';
import { noteService, type NoteDocument } from '@/services/noteService';
import { Button } from '@/components/ui/button';

interface NoteEditorClientProps {
  id: string;
}

export function NoteEditorClient({ id }: NoteEditorClientProps) {
  const [note, setNote] = useState<NoteDocument | null>(null);
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [activeView, setActiveView] = useState<'note' | 'graph'>('note');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const noteGraph = useMemo(() => buildMarkdownKnowledgeGraph(content, title || 'Nota'), [content, title]);

  useEffect(() => {
    if (!id) return;
    let cancelled = false;
    setLoading(true);
    void noteService.get(id)
      .then(document => {
        if (cancelled) return;
        setNote(document);
        setTitle(document.title);
        setContent(document.content);
        setDirty(false);
      })
      .catch(error => toast.error('Não foi possível abrir a nota', { description: String(error) }))
      .finally(() => !cancelled && setLoading(false));
    return () => { cancelled = true; };
  }, [id]);

  const save = useCallback(async () => {
    if (!note || saving || !dirty) return;
    setSaving(true);
    try {
      const saved = await noteService.save(note.id, title, content);
      setNote(saved);
      setDirty(false);
      window.dispatchEvent(new CustomEvent('notes-changed'));
      toast.success('Nota salva em Markdown');
    } catch (error) {
      toast.error('Não foi possível salvar a nota', { description: String(error) });
    } finally {
      setSaving(false);
    }
  }, [content, dirty, note, saving, title]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        void save();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [save]);

  if (loading) {
    return <div className="flex h-full items-center justify-center text-muted-foreground"><Loader2 className="h-5 w-5 animate-spin" /></div>;
  }
  if (!note) {
    return <div className="flex h-full items-center justify-center text-sm text-muted-foreground">Nota não encontrada.</div>;
  }

  return (
    <article className="document-surface">
      <header className="document-toolbar">
        <div className="segmented-control" role="tablist" aria-label="Visualização da nota">
          <button type="button" role="tab" aria-selected={activeView === 'note'} data-active={activeView === 'note'} onClick={() => setActiveView('note')}>
            <FileText /> Nota
          </button>
          <button type="button" role="tab" aria-selected={activeView === 'graph'} data-active={activeView === 'graph'} onClick={() => setActiveView('graph')}>
            <Network /> Grafo
          </button>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={() => invoke('open_meeting_folder', { meetingId: note.id, authToken: null })} title="Mostrar no Finder">
            <FolderOpen /> <span className="hidden xl:inline">Mostrar na pasta</span>
          </Button>
          <Button size="sm" onClick={() => void save()} disabled={!dirty || saving}>
            {saving ? <Loader2 className="animate-spin" /> : <Save />}
            {saving ? 'Salvando…' : dirty ? 'Salvar' : 'Salvo'}
          </Button>
        </div>
      </header>

      {activeView === 'note' ? (
        <div className="document-scroll">
          <div className="document-page">
            <input
              value={title}
              onChange={event => { setTitle(event.target.value); setDirty(true); }}
              className="document-title"
              aria-label="Título da nota"
              placeholder="Nota sem título"
            />
            <div className="document-meta"><Calendar /> {new Date(note.created_at).toLocaleDateString('pt-BR', { dateStyle: 'long' })}</div>
            <textarea
              value={content}
              onChange={event => { setContent(event.target.value); setDirty(true); }}
              placeholder="Escreva em Markdown…"
              className="markdown-editor"
              aria-label="Conteúdo da nota em Markdown"
              spellCheck
            />
          </div>
        </div>
      ) : (
        <div className="document-scroll p-4">
          <KnowledgeGraphView graph={noteGraph} title="Grafo da nota" subtitle="Temas e relações derivados localmente do documento Markdown." />
        </div>
      )}
    </article>
  );
}

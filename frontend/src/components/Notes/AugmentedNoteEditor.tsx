'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Brain, Calendar, FolderOpen, Loader2, Save } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { SkillPanel } from '@/components/Skills/SkillPanel';
import { buildSkillResultBlock, insertSkillResult, type TextSelection } from '@/lib/skillBlocks';
import { noteService, type NoteDocument } from '@/services/noteService';
import type { SkillRunResult } from '@/types/skills';

export function AugmentedNoteEditor({ id, transcript, compact = false, externalTitle, onContentChanged }: {
  id: string; transcript?: string; compact?: boolean; externalTitle?: string; onContentChanged?: (content: string, title: string) => void;
}) {
  const [note, setNote] = useState<NoteDocument | null>(null);
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [skillsOpen, setSkillsOpen] = useState(false);
  const [selection, setSelection] = useState<TextSelection | null>(null);
  const [conflict, setConflict] = useState<NoteDocument | null>(null);
  const editorRef = useRef<HTMLTextAreaElement>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try { const document = await noteService.get(id); setNote(document); setTitle(document.title); setContent(document.content); setDirty(false); setConflict(null); }
    catch (error) { toast.error('Não foi possível abrir a nota', { description: String(error) }); }
    finally { setLoading(false); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  useEffect(() => { if (compact && externalTitle) setTitle(externalTitle); }, [compact, externalTitle]);
  useEffect(() => { onContentChanged?.(content, title); }, [content, onContentChanged, title]);

  const save = useCallback(async (overwrite = false) => {
    if (!note || saving || (!dirty && !overwrite)) return;
    setSaving(true);
    try {
      const saved = await noteService.save(note.id, title, content, overwrite ? undefined : note.content_hash);
      setNote(saved); setContent(saved.content); setDirty(false); setConflict(null);
      window.dispatchEvent(new CustomEvent('notes-changed')); window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      toast.success('Nota salva em Markdown');
    } catch (error) {
      if (String(error).includes('NOTE_CONFLICT')) {
        try { setConflict(await noteService.get(note.id)); } catch { /* retain the original conflict */ }
      } else toast.error('Não foi possível salvar a nota', { description: String(error) });
    } finally { setSaving(false); }
  }, [content, dirty, note, saving, title]);

  const captureSelection = () => {
    const editor = editorRef.current;
    if (!editor) return setSelection(null);
    const text = content.slice(editor.selectionStart, editor.selectionEnd);
    setSelection(text ? { start: editor.selectionStart, end: editor.selectionEnd, text } : null);
  };
  const openSkills = () => { captureSelection(); setSkillsOpen(true); };
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void save(); }
    };
    const openFromPalette = () => openSkills();
    window.addEventListener('keydown', keydown); window.addEventListener('open-note-skills', openFromPalette);
    return () => { window.removeEventListener('keydown', keydown); window.removeEventListener('open-note-skills', openFromPalette); };
  });

  const changeContent = (next: string) => {
    if (/(^|\n)\/skill\s*$/.test(next)) { setContent(next.replace(/(^|\n)\/skill\s*$/, '$1')); setDirty(true); setSelection(null); setSkillsOpen(true); return; }
    setContent(next); setDirty(true);
  };
  const accept = (result: SkillRunResult, resultTitle: string, markdown: string) => {
    const block = buildSkillResultBlock(result, resultTitle, markdown);
    const inserted = insertSkillResult(content, block, selection);
    setContent(inserted.content); setDirty(true);
    toast.info(inserted.afterSelection ? 'Resultado inserido após a seleção' : selection ? 'A seleção mudou; resultado adicionado ao final' : 'Resultado adicionado ao final');
  };

  if (loading) return <div className="flex h-full items-center justify-center text-muted-foreground"><Loader2 className="h-5 w-5 animate-spin" /></div>;
  if (!note) return <div className="flex h-full items-center justify-center text-sm text-muted-foreground">Nota não encontrada.</div>;
  return <div className="augmented-editor" data-compact={compact}>
    <div className="augmented-note-main">
      <div className="augmented-note-actions">
        <Button variant="ghost" size="sm" onClick={openSkills}><Brain /> Skills <kbd>/skill</kbd></Button>
        {!compact && <Button variant="ghost" size="sm" onClick={() => invoke('open_meeting_folder', { meetingId: note.id, authToken: null })}><FolderOpen /> Mostrar na pasta</Button>}
        <Button size="sm" onClick={() => void save()} disabled={!dirty || saving}>{saving ? <Loader2 className="animate-spin" /> : <Save />}{saving ? 'Salvando…' : dirty ? 'Salvar' : 'Salvo'}</Button>
      </div>
      <div className="document-scroll"><div className="document-page">
        {!compact && <><input value={title} onChange={event => { setTitle(event.target.value); setDirty(true); }} className="document-title" aria-label="Título da nota" placeholder="Nota sem título" /><div className="document-meta"><Calendar /> {new Date(note.created_at).toLocaleDateString('pt-BR', { dateStyle: 'long' })}</div></>}
        {compact && <p className="augmented-note-intro">A nota é o espaço principal. A IA propõe; você revisa e decide o que incorporar.</p>}
        <textarea ref={editorRef} value={content} onChange={event => changeContent(event.target.value)} onSelect={captureSelection} placeholder="Escreva em Markdown… Digite /skill para ampliar este pensamento." className="markdown-editor" aria-label="Conteúdo da nota em Markdown" spellCheck />
      </div></div>
      {conflict && <aside className="note-conflict"><div><strong>Esta nota mudou fora do Empathy</strong><p>Compare antes de decidir. Nada foi sobrescrito.</p></div><div className="note-conflict-columns"><section><h4>Sua versão</h4><pre>{content}</pre></section><section><h4>Versão no disco</h4><pre>{conflict.content}</pre></section></div><div><Button variant="ghost" onClick={() => { setNote(conflict); setTitle(conflict.title); setContent(conflict.content); setDirty(false); setConflict(null); }}>Usar versão do disco</Button><Button onClick={() => void save(true)}>Substituir após revisão</Button></div></aside>}
    </div>
    <SkillPanel open={skillsOpen} note={{ ...note, title }} content={content} selection={selection} transcript={transcript} onClose={() => setSkillsOpen(false)} onAccept={accept} />
  </div>;
}

'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Brain, Download, FilePlus2, Loader2, Sparkles, Trash2, Upload, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useConfig } from '@/contexts/ConfigContext';
import type { NoteDocument } from '@/services/noteService';
import type { ExternalSkillDocument, RelatedSkillDocument, SkillDefinition, SkillInfo, SkillLayer, SkillRunResult } from '@/types/skills';
import type { TextSelection } from '@/lib/skillBlocks';
import { OutlookContextPicker } from '@/components/Skills/OutlookContextPicker';

type RelatedMeeting = { meeting_id: string; title: string; reasons: string[] };
const labels: Record<SkillLayer, string> = { individual: 'Individual', collective: 'Coletiva', artificial: 'Artificial' };

export function SkillPanel({ open, note, content, selection, transcript, sourceMode = 'note', onClose, onAccept }: {
  open: boolean; note: NoteDocument; content: string; selection?: TextSelection | null; transcript?: string;
  sourceMode?: 'note' | 'transcript'; onClose: () => void;
  onAccept: (result: SkillRunResult, title: string, markdown: string) => void | Promise<void>;
}) {
  const { modelConfig } = useConfig();
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [selectedId, setSelectedId] = useState('clarify-thinking');
  const [layer, setLayer] = useState<SkillLayer | 'all'>('all');
  const [useTranscript, setUseTranscript] = useState(false);
  const [related, setRelated] = useState<RelatedMeeting[]>([]);
  const [relatedIds, setRelatedIds] = useState<string[]>([]);
  const [status, setStatus] = useState('');
  const [runId, setRunId] = useState<string | null>(null);
  const [result, setResult] = useState<SkillRunResult | null>(null);
  const [reviewTitle, setReviewTitle] = useState('');
  const [reviewMarkdown, setReviewMarkdown] = useState('');
  const [editor, setEditor] = useState<SkillDefinition | null>(null);
  const [privacyAccepted, setPrivacyAccepted] = useState(false);
  const [externalDocuments, setExternalDocuments] = useState<ExternalSkillDocument[]>([]);
  const updateExternalDocuments = useCallback((documents: ExternalSkillDocument[]) => setExternalDocuments(documents), []);
  const isExternal = !['builtin-ai', 'ollama'].includes(modelConfig.provider);
  const filtered = useMemo(() => skills.filter(skill =>
    (layer === 'all' || skill.layer === layer) &&
    (sourceMode !== 'transcript' || Boolean(selection?.text) || skill.context.transcript)
  ), [layer, selection?.text, skills, sourceMode]);
  const selected = filtered.find(skill => skill.id === selectedId);
  const transcriptIsPrimary = sourceMode === 'transcript' && !selection?.text;
  const includeTranscript = transcriptIsPrimary || useTranscript;

  const reload = () => invoke<SkillInfo[]>('api_list_skills').then(setSkills);
  useEffect(() => { if (open) void reload(); }, [open]);
  useEffect(() => {
    if (!open) return;
    void invoke<RelatedMeeting[]>('api_get_related_meetings', { meetingId: note.id }).then(setRelated).catch(() => setRelated([]));
  }, [note.id, open]);
  useEffect(() => {
    let cleanup: undefined | (() => void);
    void listen<{ run_id: string; status: string; message: string }>('skill-progress', event => {
      setRunId(event.payload.run_id); setStatus(event.payload.message);
    }).then(unlisten => { cleanup = unlisten; });
    return () => cleanup?.();
  }, []);
  useEffect(() => { if (selected && !selected.context.transcript) setUseTranscript(false); }, [selected]);
  useEffect(() => {
    if (!open || sourceMode !== 'transcript') return;
    setSelectedId(selection?.text ? 'clarify-thinking' : 'meeting-summary');
    setUseTranscript(!selection?.text);
  }, [open, selection?.text, sourceMode]);
  useEffect(() => {
    if (open && note.external_meeting?.provider === 'microsoft' && sourceMode === 'note') setSelectedId('prepare-meeting');
  }, [note.external_meeting, open, sourceMode]);
  useEffect(() => {
    if (filtered.length > 0 && !filtered.some(skill => skill.id === selectedId)) setSelectedId(filtered[0].id);
  }, [filtered, selectedId]);
  useEffect(() => { setPrivacyAccepted(false); }, [selectedId, useTranscript, relatedIds, externalDocuments]);
  useEffect(() => {
    if (!open || !selected?.context.external_documents) setExternalDocuments([]);
  }, [open, selected?.context.external_documents]);
  if (!open) return null;

  const run = async () => {
    if (!selected) return;
    setResult(null); setStatus('Preparando contexto…');
    try {
      const relatedDocuments = (await Promise.all(relatedIds.slice(0, 5).map(id => invoke<NoteDocument>('api_get_note', { meetingId: id }).catch(() => null))))
        .filter((item): item is NoteDocument => Boolean(item)).map<RelatedSkillDocument>(item => ({ id: item.id, title: item.title, content: item.content }));
      const generated = await invoke<SkillRunResult>('api_run_skill', { skillId: selected.id, context: {
        note_id: note.id, note_title: note.title, note: transcriptIsPrimary ? '' : content, selection: selection?.text || null,
        transcript: includeTranscript ? transcript || null : null, related_notes: relatedDocuments,
        external_documents: selected.context.external_documents ? externalDocuments : [],
      } });
      setResult(generated); setReviewTitle(generated.title); setReviewMarkdown(generated.markdown); setStatus('Revise antes de adicionar');
    } catch (error) { setStatus(String(error)); }
  };

  const saveCustom = async () => {
    if (!editor) return;
    await invoke('api_save_custom_skill', { skill: editor }); setEditor(null); await reload();
  };
  const duplicate = (skill: SkillInfo) => setEditor({ ...skill, id: `${skill.id}-custom`, name: `${skill.name} personalizada` });

  return (
    <aside className="skill-panel" aria-label="Painel de Skills">
      <header><div><p className="eyebrow">Inteligência aumentada</p><h2><Brain /> Skills</h2></div><Button size="icon" variant="ghost" onClick={onClose} aria-label="Fechar painel"><X /></Button></header>
      <p className="skill-triad">Individual + coletiva + artificial para ampliar o pensamento humano.</p>
      {!result && !editor && <>
        <div className="skill-layer-filter" role="tablist" aria-label="Camada de inteligência">
          {(['all', 'individual', 'collective', 'artificial'] as const).map(value => <button key={value} data-active={layer === value} onClick={() => setLayer(value)}>{value === 'all' ? 'Todas' : labels[value]}</button>)}
        </div>
        <div className="skill-list">{filtered.map(skill => <button key={skill.id} data-active={selectedId === skill.id} onClick={() => setSelectedId(skill.id)}><span data-layer={skill.layer} /><strong>{skill.name}</strong><small>{skill.description}</small></button>)}{filtered.length === 0 && <p className="skill-empty">Nenhuma Skill desta camada aceita o contexto selecionado.</p>}</div>
        {selected && <section className="skill-context">
          <div><strong>Contexto desta execução</strong><small>{selection?.text ? `${sourceMode === 'transcript' ? 'Seleção da transcrição' : 'Seleção'}: “${selection.text.slice(0, 100)}${selection.text.length > 100 ? '…' : ''}”` : transcriptIsPrimary ? 'Transcrição completa' : 'Nota completa'}</small></div>
          {transcriptIsPrimary && <p className="skill-context-fixed">A Skill usará a transcrição como fonte e criará uma proposta para a Nota.</p>}
          {!transcriptIsPrimary && selected.context.transcript && transcript && <label><input type="checkbox" checked={useTranscript} onChange={event => setUseTranscript(event.target.checked)} /> Incluir transcrição</label>}
          {selected.context.related_notes && related.length > 0 && <details><summary>Escolher notas relacionadas ({relatedIds.length}/5)</summary>{related.slice(0, 10).map(item => <label key={item.meeting_id}><input type="checkbox" checked={relatedIds.includes(item.meeting_id)} disabled={!relatedIds.includes(item.meeting_id) && relatedIds.length >= 5} onChange={event => setRelatedIds(ids => event.target.checked ? [...ids, item.meeting_id] : ids.filter(id => id !== item.meeting_id))} /> <span>{item.title}<small>{item.reasons.join(' · ')}</small></span></label>)}</details>}
        </section>}
        {selected?.context.external_documents && note.external_meeting?.provider === 'microsoft' && <OutlookContextPicker meeting={note.external_meeting} onDocumentsChange={updateExternalDocuments} />}
        <div className="skill-privacy" data-external={isExternal}><strong>{isExternal ? 'Processamento externo' : 'Processamento local'}</strong><span>{modelConfig.provider} · {modelConfig.model}</span><p>{isExternal ? `Será enviado: ${selection?.text ? 'seleção' : transcriptIsPrimary ? 'transcrição completa' : 'nota completa'}${!transcriptIsPrimary && useTranscript ? ', transcrição' : ''}${relatedIds.length ? `, ${relatedIds.length} nota(s) relacionada(s)` : ''}${externalDocuments.length ? ` e ${externalDocuments.length} e-mail(s) selecionado(s): ${externalDocuments.map(document => document.title).join('; ')}` : ''}.` : externalDocuments.length ? `O conteúdo da nota e ${externalDocuments.length} e-mail(s) selecionado(s) será processado neste dispositivo.` : 'O conteúdo permanece neste dispositivo.'}</p>{isExternal && <label><input type="checkbox" checked={privacyAccepted} onChange={event => setPrivacyAccepted(event.target.checked)} /> Confirmo o envio deste contexto exato</label>}</div>
        <div className="skill-actions"><Button variant="ghost" onClick={() => selected && duplicate(selected)}><FilePlus2 /> Duplicar</Button><Button onClick={() => void run()} disabled={!selected || (isExternal && !privacyAccepted)}><Sparkles /> Executar Skill</Button></div>
        <details className="skill-library"><summary>Gerenciar biblioteca</summary><div><Button size="sm" variant="ghost" onClick={() => setEditor({ schema: 1, id: 'nova-skill', name: 'Nova Skill', description: 'Como esta Skill amplia o pensamento.', layer: 'individual', instruction: '', default_title: 'Novo resultado', context: { selection: true, note: true, transcript: false, related_notes: false, external_documents: false } })}><FilePlus2 /> Criar</Button><Button size="sm" variant="ghost" onClick={() => void invoke('api_import_skill', { filePath: null }).then(reload)}><Upload /> Importar</Button><Button size="sm" variant="ghost" onClick={() => void invoke<number>('api_migrate_custom_templates').then(count => { setStatus(`${count} template(s) adaptado(s); originais preservados.`); return reload(); })}>Adaptar templates antigos</Button>{selected && <Button size="sm" variant="ghost" onClick={() => void invoke('api_export_skill', { skillId: selected.id, filePath: null })}><Download /> Exportar</Button>}{selected && !selected.native && <Button size="sm" variant="ghost" onClick={() => setEditor(selected)}>Editar</Button>}{selected && !selected.native && <Button size="sm" variant="ghost" onClick={() => { if (window.confirm(`Excluir a Skill “${selected.name}”?`)) void invoke('api_delete_custom_skill', { skillId: selected.id }).then(reload); }}><Trash2 /> Excluir</Button>}</div></details>
      </>}
      {editor && <div className="skill-editor"><h3>Skill personalizada</h3><label>Nome<input value={editor.name} onChange={e => setEditor({ ...editor, name: e.target.value })} /></label><label>Identificador<input disabled={skills.some(skill => !skill.native && skill.id === editor.id)} value={editor.id} onChange={e => setEditor({ ...editor, id: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '-') })} /></label><label>Camada<select value={editor.layer} onChange={e => setEditor({ ...editor, layer: e.target.value as SkillLayer })}><option value="individual">Individual</option><option value="collective">Coletiva</option><option value="artificial">Artificial</option></select></label><label>Descrição<input value={editor.description} onChange={e => setEditor({ ...editor, description: e.target.value })} /></label><label>Instrução<textarea value={editor.instruction} onChange={e => setEditor({ ...editor, instruction: e.target.value })} /></label><label>Título padrão<input value={editor.default_title} onChange={e => setEditor({ ...editor, default_title: e.target.value })} /></label><label className="skill-editor-check"><input type="checkbox" checked={editor.context.transcript} onChange={e => setEditor({ ...editor, context: { ...editor.context, transcript: e.target.checked } })} /> Permitir transcrição opcional</label><label className="skill-editor-check"><input type="checkbox" checked={editor.context.related_notes} onChange={e => setEditor({ ...editor, context: { ...editor.context, related_notes: e.target.checked } })} /> Permitir notas relacionadas</label><label className="skill-editor-check"><input type="checkbox" checked={editor.context.external_documents} onChange={e => setEditor({ ...editor, context: { ...editor.context, external_documents: e.target.checked } })} /> Permitir documentos externos escolhidos</label><div className="skill-actions"><Button variant="ghost" onClick={() => setEditor(null)}>Cancelar</Button><Button onClick={() => void saveCustom()}>Salvar Skill</Button></div></div>}
      {result && <div className="skill-review"><div><p className="eyebrow">Revisão humana</p><h3>Edite a proposta antes de incorporar</h3></div><label>Título<input value={reviewTitle} onChange={e => setReviewTitle(e.target.value)} /></label><label>Markdown<textarea value={reviewMarkdown} onChange={e => setReviewMarkdown(e.target.value)} /></label><small>{result.provider} · {result.model} · nova versão</small><div className="skill-actions"><Button variant="ghost" onClick={() => { setResult(null); setStatus(''); }}>Descartar</Button><Button onClick={() => void Promise.resolve(onAccept(result, reviewTitle, reviewMarkdown)).then(onClose).catch(error => setStatus(String(error)))}><Sparkles /> Adicionar à nota</Button></div></div>}
      {status && !result && <div className="skill-status">{runId && status.includes('elaborando') && <Loader2 className="animate-spin" />}<span>{status}</span>{runId && <Button size="sm" variant="ghost" onClick={() => void invoke('api_cancel_skill', { runId })}>Cancelar</Button>}</div>}
    </aside>
  );
}

'use client';

import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { GitMerge, Loader2, Pencil, Trash2, UserRoundPlus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { OutlookNoteMeetingContext } from '@/types/integrations';
import type { ParticipantMemory, ParticipantMemoryUpdate } from '@/types/participants';

function toUpdate(memory: ParticipantMemory): ParticipantMemoryUpdate {
  return {
    id: memory.id,
    display_name: memory.display_name,
    emails: memory.emails,
    aliases: memory.aliases,
    organization: memory.organization,
    role: memory.role,
    notes: memory.notes,
    hypotheses: memory.hypotheses,
    expected_updated_at: memory.updated_at,
  };
}

export function ParticipantMemoryPanel({ noteId, meeting }: { noteId: string; meeting: OutlookNoteMeetingContext }) {
  const [memories, setMemories] = useState<ParticipantMemory[]>([]);
  const [selectedEmails, setSelectedEmails] = useState<string[]>([]);
  const [editing, setEditing] = useState<ParticipantMemoryUpdate | null>(null);
  const [mergeTarget, setMergeTarget] = useState('');
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');

  const participants = useMemo(() => {
    const unique = new Map<string, { display_name: string; email: string }>();
    for (const participant of [meeting.organizer, ...meeting.attendees]) {
      if (participant?.email) unique.set(participant.email.toLowerCase(), participant);
    }
    return [...unique.values()];
  }, [meeting.attendees, meeting.organizer]);
  const matchingMemories = useMemo(() => memories.filter(memory => memory.emails.some(email => participants.some(participant => participant.email.toLowerCase() === email.toLowerCase()))), [memories, participants]);

  const reload = async () => setMemories(await invoke<ParticipantMemory[]>('api_list_participant_memories'));
  useEffect(() => { void reload().catch(error => setStatus(String(error))); }, []);

  const confirm = async () => {
    if (!selectedEmails.length) return;
    setBusy(true); setStatus('');
    try {
      setMemories(await invoke<ParticipantMemory[]>('api_confirm_note_participants', { meetingId: noteId, emails: selectedEmails }));
      setSelectedEmails([]);
      window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      setStatus('Identidades adicionadas à memória local após sua confirmação.');
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(false); }
  };

  const save = async () => {
    if (!editing) return;
    setBusy(true); setStatus('');
    try {
      const saved = await invoke<ParticipantMemory>('api_save_participant_memory', { update: editing });
      setMemories(current => current.map(memory => memory.id === saved.id ? saved : memory));
      setEditing(null);
      window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      setStatus('Memória corrigida no arquivo Markdown.');
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(false); }
  };

  const remove = async (memory: ParticipantMemory) => {
    if (!window.confirm(`Remover “${memory.display_name}” da memória? O arquivo será movido para a lixeira recuperável do workspace.`)) return;
    setBusy(true); setStatus('');
    try {
      await invoke('api_delete_participant_memory', { participantId: memory.id });
      setMemories(current => current.filter(item => item.id !== memory.id));
      if (editing?.id === memory.id) setEditing(null);
      window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      setStatus('Memória movida para a lixeira recuperável.');
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(false); }
  };

  const merge = async () => {
    if (!editing || !mergeTarget) return;
    const target = memories.find(memory => memory.id === mergeTarget);
    if (!target || !window.confirm(`Mesclar “${editing.display_name}” em “${target.display_name}”? O primeiro arquivo irá para a lixeira recuperável.`)) return;
    setBusy(true); setStatus('');
    try {
      await invoke<ParticipantMemory>('api_merge_participant_memories', { sourceId: editing.id, targetId: mergeTarget });
      setEditing(null); setMergeTarget(''); await reload();
      window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      setStatus('Identidades mescladas; as fontes foram preservadas.');
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(false); }
  };

  return <details className="participant-memory-panel">
    <summary>Memória local de participantes <span>{matchingMemories.length}/{participants.length}</span></summary>
    <p>Somente você decide quais identidades entram na memória. Nenhum perfil é criado automaticamente.</p>
    <div className="participant-memory-candidates">{participants.map(participant => {
      const exists = memories.some(memory => memory.emails.some(email => email.toLowerCase() === participant.email.toLowerCase()));
      return <label key={participant.email} data-confirmed={exists}><input type="checkbox" disabled={exists || busy} checked={exists || selectedEmails.includes(participant.email)} onChange={event => setSelectedEmails(current => event.target.checked ? [...current, participant.email] : current.filter(email => email !== participant.email))} /><span>{participant.display_name}<small>{participant.email}{exists ? ' · na memória' : ''}</small></span></label>;
    })}</div>
    {selectedEmails.length > 0 && <Button size="sm" onClick={() => void confirm()} disabled={busy}>{busy ? <Loader2 className="animate-spin" /> : <UserRoundPlus />}Adicionar {selectedEmails.length} após confirmar</Button>}
    {matchingMemories.length > 0 && <div className="participant-memory-list">{matchingMemories.map(memory => <button key={memory.id} onClick={() => { setEditing(toUpdate(memory)); setMergeTarget(''); }}><span><strong>{memory.display_name}</strong><small>{memory.organization || memory.role ? [memory.role, memory.organization].filter(Boolean).join(' · ') : `${memory.source_receipts.length} fonte(s) confirmada(s)`}</small></span><Pencil /></button>)}</div>}
    {editing && <div className="participant-memory-editor"><h4>Corrigir memória</h4><label>Nome<input value={editing.display_name} onChange={event => setEditing({ ...editing, display_name: event.target.value })} /></label><label>E-mails<input value={editing.emails.join(', ')} onChange={event => setEditing({ ...editing, emails: event.target.value.split(',').map(value => value.trim()).filter(Boolean) })} /></label><label>Apelidos<input value={editing.aliases.join(', ')} onChange={event => setEditing({ ...editing, aliases: event.target.value.split(',').map(value => value.trim()).filter(Boolean) })} /></label><div><label>Organização<input value={editing.organization || ''} onChange={event => setEditing({ ...editing, organization: event.target.value })} /></label><label>Papel<input value={editing.role || ''} onChange={event => setEditing({ ...editing, role: event.target.value })} /></label></div><label>Contexto confirmado<textarea value={editing.notes} onChange={event => setEditing({ ...editing, notes: event.target.value })} placeholder="Somente fatos que você deseja registrar." /></label><label className="participant-hypothesis">Hipóteses a revisar<textarea value={editing.hypotheses} onChange={event => setEditing({ ...editing, hypotheses: event.target.value })} placeholder="Interpretações ainda não confirmadas." /></label><div className="participant-memory-actions"><Button size="sm" variant="ghost" onClick={() => setEditing(null)}>Cancelar</Button><Button size="sm" onClick={() => void save()} disabled={busy}>Salvar correção</Button><Button size="icon" variant="ghost" aria-label="Remover memória" onClick={() => { const memory = memories.find(item => item.id === editing.id); if (memory) void remove(memory); }}><Trash2 /></Button></div><div className="participant-memory-merge"><select value={mergeTarget} onChange={event => setMergeTarget(event.target.value)}><option value="">Mesclar com…</option>{memories.filter(memory => memory.id !== editing.id).map(memory => <option key={memory.id} value={memory.id}>{memory.display_name}</option>)}</select><Button size="sm" variant="outline" disabled={!mergeTarget || busy} onClick={() => void merge()}><GitMerge />Mesclar</Button></div></div>}
    {status && <p className="participant-memory-status" role="status">{status}</p>}
  </details>;
}

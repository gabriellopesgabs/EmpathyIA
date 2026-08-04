'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChevronDown, Tags } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

type MeetingProperties = {
  project?: string;
  participants: string[];
  tags: string[];
  status?: string;
};

const emptyProperties: MeetingProperties = { project: '', participants: [], tags: ['meeting'], status: 'completed' };

export function MeetingPropertiesEditor({ meetingId }: { meetingId: string }) {
  const [open, setOpen] = useState(false);
  const [properties, setProperties] = useState<MeetingProperties>(emptyProperties);
  const [participants, setParticipants] = useState('');
  const [tags, setTags] = useState('meeting');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const load = () => invoke<MeetingProperties>('api_get_meeting_properties', { meetingId })
      .then(value => {
        if (cancelled) return;
        setProperties(value);
        setParticipants(value.participants.join(', '));
        setTags(value.tags.join(', '));
      })
      .catch(error => console.warn('Meeting properties are not available yet:', error));
    load();
    window.addEventListener('knowledge-index-updated', load);
    return () => {
      cancelled = true;
      window.removeEventListener('knowledge-index-updated', load);
    };
  }, [meetingId]);

  const save = async () => {
    setSaving(true);
    try {
      const next = {
        ...properties,
        participants: participants.split(',').map(value => value.trim()).filter(Boolean),
        tags: tags.split(',').map(value => value.trim()).filter(Boolean),
      };
      await invoke('api_update_meeting_properties', { meetingId, properties: next });
      await invoke('api_reindex_knowledge');
      setProperties(next);
      toast.success('Propriedades salvas no meeting.md');
      setOpen(false);
    } catch (error) {
      toast.error('Não foi possível salvar as propriedades', { description: String(error) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="mt-3 rounded-lg border bg-gray-50">
      <button className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-gray-600" onClick={() => setOpen(value => !value)}>
        <Tags className="h-3.5 w-3.5" />
        <span>{properties.project || 'Adicionar projeto, participantes e tags'}</span>
        {properties.participants.length > 0 && <span className="text-gray-400">· {properties.participants.join(', ')}</span>}
        <ChevronDown className={`ml-auto h-3.5 w-3.5 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>
      {open && (
        <div className="grid gap-2 border-t p-3 sm:grid-cols-2">
          <Input className="h-8 text-xs" placeholder="Projeto" value={properties.project || ''}
            onChange={event => setProperties({ ...properties, project: event.target.value })} />
          <Input className="h-8 text-xs" placeholder="Participantes separados por vírgula" value={participants}
            onChange={event => setParticipants(event.target.value)} />
          <Input className="h-8 text-xs" placeholder="Tags separadas por vírgula" value={tags}
            onChange={event => setTags(event.target.value)} />
          <div className="flex gap-2">
            <select className="h-8 flex-1 rounded-md border bg-white px-2 text-xs" value={properties.status || 'active'}
              onChange={event => setProperties({ ...properties, status: event.target.value })}>
              <option value="active">Ativa</option>
              <option value="completed">Concluída</option>
              <option value="review">Revisar</option>
              <option value="archived">Arquivada</option>
            </select>
            <Button size="sm" onClick={save} disabled={saving}>{saving ? 'Salvando…' : 'Salvar'}</Button>
          </div>
        </div>
      )}
    </div>
  );
}

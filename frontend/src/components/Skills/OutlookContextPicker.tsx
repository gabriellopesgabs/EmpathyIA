'use client';

import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, Loader2, Mail, ShieldCheck } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type {
  ConnectedAccount,
  OutlookMailCandidate,
  OutlookNoteMeetingContext,
  OutlookSelectedMail,
} from '@/types/integrations';
import type { ExternalSkillDocument } from '@/types/skills';

export function OutlookContextPicker({ meeting, onDocumentsChange }: {
  meeting: OutlookNoteMeetingContext;
  onDocumentsChange: (documents: ExternalSkillDocument[]) => void;
}) {
  const [account, setAccount] = useState<ConnectedAccount | null>(null);
  const [accountLoaded, setAccountLoaded] = useState(false);
  const [participantEmails, setParticipantEmails] = useState<string[]>([]);
  const [candidates, setCandidates] = useState<OutlookMailCandidate[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [loaded, setLoaded] = useState<OutlookSelectedMail[]>([]);
  const [busy, setBusy] = useState<'search' | 'content' | null>(null);
  const [status, setStatus] = useState('');

  const participants = useMemo(() => {
    const entries = [meeting.organizer, ...meeting.attendees].filter((item): item is NonNullable<typeof item> => Boolean(item?.email) && item?.email.toLowerCase() !== account?.email.toLowerCase());
    const unique = new Map<string, typeof entries[number]>();
    for (const entry of entries) unique.set(entry.email.toLowerCase(), entry);
    return [...unique.values()];
  }, [account?.email, meeting.attendees, meeting.organizer]);

  useEffect(() => {
    let active = true;
    void invoke<ConnectedAccount[]>('api_list_connected_accounts').then(accounts => {
      if (active) { setAccount(accounts.find(item => item.id === meeting.account_id) || null); setAccountLoaded(true); }
    }).catch(error => { if (active) { setStatus(String(error)); setAccountLoaded(true); } });
    return () => { active = false; onDocumentsChange([]); };
  }, [meeting.account_id, onDocumentsChange]);

  const clearLoaded = () => {
    setLoaded([]);
    onDocumentsChange([]);
  };

  const toggleParticipant = (email: string, checked: boolean) => {
    setParticipantEmails(current => checked ? [...current, email] : current.filter(item => item !== email));
    setCandidates([]); setSelectedIds([]); clearLoaded(); setStatus('');
  };

  const search = async () => {
    if (!account || participantEmails.length === 0) return;
    setBusy('search'); setStatus(''); setCandidates([]); setSelectedIds([]); clearLoaded();
    try {
      let authorized = account;
      if (!authorized.granted_permissions.some(permission => permission === 'mail.metadata' || permission === 'mail.content')) {
        setStatus('Autorize somente os metadados no navegador da Microsoft.');
        authorized = await invoke<ConnectedAccount>('api_authorize_microsoft_mail', {
          accountId: account.id,
          includeContent: false,
        });
        setAccount(authorized);
      }
      const messages = await invoke<OutlookMailCandidate[]>('api_search_outlook_mail_context', {
        accountId: authorized.id,
        eventId: meeting.calendar_event_id,
        participantEmails,
        limit: 25,
      });
      setCandidates(messages);
      setStatus(messages.length ? 'Escolha até 10 mensagens. O conteúdo ainda não foi lido.' : 'Nenhuma mensagem encontrada para os participantes escolhidos.');
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(null); }
  };

  const toggleCandidate = (id: string, checked: boolean) => {
    setSelectedIds(current => checked ? [...current, id] : current.filter(item => item !== id));
    clearLoaded();
  };

  const loadSelected = async () => {
    if (!account || selectedIds.length === 0) return;
    setBusy('content'); setStatus(''); clearLoaded();
    try {
      let authorized = account;
      if (!authorized.granted_permissions.includes('mail.content')) {
        setStatus('Para ler somente as mensagens escolhidas, autorize Mail.Read no navegador.');
        authorized = await invoke<ConnectedAccount>('api_authorize_microsoft_mail', {
          accountId: account.id,
          includeContent: true,
        });
        setAccount(authorized);
      }
      const messages = await invoke<OutlookSelectedMail[]>('api_get_selected_outlook_mail', {
        accountId: authorized.id,
        messageIds: selectedIds,
      });
      setLoaded(messages);
      onDocumentsChange(messages.map(message => ({
        id: message.source_receipt.source_id,
        title: message.subject,
        content: message.body_text,
        source_kind: 'mail-message',
        provider: 'microsoft',
        occurred_at: message.source_receipt.occurred_at,
      })));
      setStatus(`${messages.length} mensagem(ns) carregada(s) apenas para esta execução.`);
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(null); }
  };

  if (!accountLoaded) return <section className="outlook-context-picker"><div className="outlook-context-heading"><Loader2 className="animate-spin" /><div><strong>Contexto do Outlook</strong><small>Verificando a conta conectada…</small></div></div></section>;
  if (!account) return <section className="outlook-context-picker"><div className="outlook-context-heading"><Mail /><div><strong>Contexto do Outlook</strong><small>Esta conta não está mais conectada. Reconecte-a em Ajustes.</small></div></div></section>;

  return <section className="outlook-context-picker" aria-label="Contexto selecionado do Outlook">
    <div className="outlook-context-heading"><Mail /><div><strong>Contexto de e-mails</strong><small>Escolha pessoas, veja metadados e só então autorize o conteúdo.</small></div></div>
    <div className="outlook-context-participants"><span>1. Participantes</span>{participants.map(participant => <label key={participant.email}><input type="checkbox" checked={participantEmails.includes(participant.email)} onChange={event => toggleParticipant(participant.email, event.target.checked)} /><span>{participant.display_name}<small>{participant.email}</small></span></label>)}</div>
    <Button size="sm" variant="outline" disabled={busy !== null || participantEmails.length === 0} onClick={() => void search()}>{busy === 'search' ? <Loader2 className="animate-spin" /> : <ShieldCheck />}{account.granted_permissions.some(permission => permission === 'mail.metadata' || permission === 'mail.content') ? 'Buscar metadados' : 'Autorizar metadados e buscar'}</Button>
    {candidates.length > 0 && <div className="outlook-context-messages"><span>2. Mensagens encontradas</span>{candidates.map(message => <label key={message.id}><input type="checkbox" checked={selectedIds.includes(message.id)} disabled={!selectedIds.includes(message.id) && selectedIds.length >= 10} onChange={event => toggleCandidate(message.id, event.target.checked)} /><span><strong>{message.subject}</strong><small>{message.sender?.display_name || 'Remetente não informado'}{message.sent_at ? ` · ${new Date(message.sent_at).toLocaleDateString('pt-BR')}` : ''}</small></span></label>)}</div>}
    {selectedIds.length > 0 && <Button size="sm" disabled={busy !== null} onClick={() => void loadSelected()}>{busy === 'content' ? <Loader2 className="animate-spin" /> : <Mail />}{account.granted_permissions.includes('mail.content') ? `Carregar ${selectedIds.length} selecionada(s)` : `Autorizar conteúdo de ${selectedIds.length}`}</Button>}
    {loaded.length > 0 && <div className="outlook-context-loaded"><span><Check /> Contexto exato desta execução</span>{loaded.map(message => <details key={message.id}><summary>{message.subject}</summary><pre>{message.body_text}</pre></details>)}</div>}
    {status && <p className="outlook-context-status" role="status">{status}</p>}
  </section>;
}

'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Bot, CircleAlert, Loader2, LogOut, RefreshCw, ShieldCheck, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { AgentServiceReadiness, MeetingAgentAudit, MeetingAgentEventType, OutlookNoteMeetingContext } from '@/types/integrations';

const stateLabels: Record<MeetingAgentEventType, string> = {
  planned: 'Solicitado', invited: 'Convidado', waiting: 'Na sala de espera', joined: 'Presente',
  'consent-requested': 'Consentimento solicitado', 'consent-granted': 'Consentimento concedido',
  'consent-denied': 'Consentimento negado', transcribing: 'Transcrevendo', paused: 'Pausado',
  leaving: 'Saindo', left: 'Saiu', error: 'Erro',
};

export function MeetingAgentPanel({ noteId, meeting, onClose }: { noteId: string; meeting: OutlookNoteMeetingContext; onClose: () => void }) {
  const [readiness, setReadiness] = useState<AgentServiceReadiness | null>(null);
  const [audit, setAudit] = useState<MeetingAgentAudit | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    const [nextReadiness, nextAudit] = await Promise.all([
      invoke<AgentServiceReadiness>('api_get_agent_service_readiness'),
      invoke<MeetingAgentAudit>('api_get_meeting_agent_audit', { meetingId: noteId }),
    ]);
    setReadiness(nextReadiness); setAudit(nextAudit);
  }, [noteId]);
  useEffect(() => { void load().catch(reason => setError(String(reason))); }, [load]);
  useEffect(() => {
    if (!audit?.current_state || ['left', 'error', 'consent-denied'].includes(audit.current_state)) return;
    const timer = window.setInterval(() => void invoke<MeetingAgentAudit>('api_refresh_teams_agent', { meetingId: noteId }).then(setAudit).catch(reason => setError(String(reason))), 5000);
    return () => window.clearInterval(timer);
  }, [audit?.current_state, noteId]);

  const invite = async () => {
    setBusy(true); setError('');
    try { setAudit(await invoke<MeetingAgentAudit>('api_request_teams_agent', { meetingId: noteId, requesterConfirmedVisibleDisclosure: confirmed })); setConfirmed(false); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };
  const refresh = async () => {
    setBusy(true); setError('');
    try { setAudit(await invoke<MeetingAgentAudit>('api_refresh_teams_agent', { meetingId: noteId })); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };
  const leave = async () => {
    setBusy(true); setError('');
    try { setAudit(await invoke<MeetingAgentAudit>('api_leave_teams_agent', { meetingId: noteId })); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };

  const canInvite = !audit?.current_state || audit.current_state === 'left';
  const canLeave = Boolean(audit?.current_state && audit.current_state !== 'left');
  return <aside className="skill-panel meeting-agent-panel" aria-label="Agente Empathy para a reunião">
    <header><div><p className="eyebrow">Participante visível</p><h2><Bot /> Agente Empathy</h2></div><Button size="icon" variant="ghost" onClick={onClose} aria-label="Fechar painel"><X /></Button></header>
    <div className="meeting-agent-identity"><Bot /><div><strong>{readiness?.visible_name || 'Empathy AI — gravação e transcrição'}</strong><small>O agente aparece com este nome para todas as pessoas.</small></div></div>
    {meeting.meeting_provider !== 'microsoft-teams' && <div className="meeting-agent-warning"><CircleAlert /> Esta reunião não é do Microsoft Teams.</div>}
    {readiness && !readiness.ready && <section className="meeting-agent-readiness"><strong>Serviço ainda não disponível</strong><p>{readiness.service_error || 'A configuração de produção não foi concluída.'}</p>{readiness.missing.length > 0 && <ul>{readiness.missing.map(item => <li key={item}>{item}</li>)}</ul>}<small>Configure e valide o serviço em Ajustes → Integrações.</small></section>}
    {readiness?.ready && canInvite && <section className="meeting-agent-consent"><ShieldCheck /><div><strong>Antes de convidar</strong><p>O agente entrará como participante visível. Isso não afirma que as outras pessoas consentiram com transcrição; ele solicitará e registrará esse consentimento depois de entrar.</p><label><input type="checkbox" checked={confirmed} onChange={event => setConfirmed(event.target.checked)} /> Confirmo que quero convidar este agente visível</label><Button disabled={!confirmed || busy} onClick={() => void invite()}>{busy ? <Loader2 className="animate-spin" /> : <Bot />}Convidar agente</Button></div></section>}
    {audit && audit.events.length > 0 && <section className="meeting-agent-timeline"><header><strong>Trilha de auditoria</strong><div><Button size="icon" variant="ghost" aria-label="Atualizar estado" disabled={busy} onClick={() => void refresh()}><RefreshCw /></Button>{canLeave && <Button size="sm" variant="ghost" disabled={busy} onClick={() => void leave()}><LogOut /> Retirar agente</Button>}</div></header><ol>{audit.events.map(event => <li key={event.event_id} data-state={event.state}><span /><div><strong>{stateLabels[event.state]}</strong><time dateTime={event.occurred_at}>{new Date(event.occurred_at).toLocaleString('pt-BR')}</time>{event.details && <p>{event.details}</p>}{event.state === 'transcribing' && <small>{event.recording_status_confirmed ? 'Estado de gravação confirmado pelo provedor' : 'Bloqueado: estado de gravação ausente'}</small>}</div></li>)}</ol><small>Arquivo portátil: agent-audit.md</small></section>}
    {error && <div className="integration-error" role="alert">{error}</div>}
  </aside>;
}

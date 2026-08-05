'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useRouter } from 'next/navigation';
import { CalendarDays, ExternalLink, Mail, Server, ShieldCheck, Unplug, UsersRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { AgentServiceReadiness, ConnectedAccount, IntegrationCapability, IntegrationFeatureFlags, IntegrationProvider, MicrosoftAuthReadiness, OutlookCalendarEvent, PreparedOutlookNote, ProviderAdapterReadiness } from '@/types/integrations';

const providerNames: Record<IntegrationProvider, string> = {
  microsoft: 'Microsoft Outlook',
  'microsoft-teams': 'Microsoft Teams',
  zoom: 'Zoom',
  'google-meet': 'Google Meet',
};

const stageLabels: Record<IntegrationCapability['stage'], string> = {
  'local-ready': 'Disponível localmente',
  'provider-setup': 'Requer configuração do provedor',
  'admin-consent': 'Requer consentimento administrativo',
  'external-review': 'Requer revisão da plataforma',
  'developer-preview': 'Developer Preview',
};

const capabilityIcons: Record<IntegrationCapability['id'], typeof CalendarDays> = {
  outlook_calendar: CalendarDays,
  outlook_mail_context: Mail,
  teams_agent: UsersRound,
  zoom_rtms: UsersRound,
  google_meet: CalendarDays,
  google_meet_media_preview: UsersRound,
};

const adapterModeLabels: Record<ProviderAdapterReadiness['mode'], string> = {
  'hosted-visible-agent': 'Participante visível',
  'realtime-media-stream': 'Stream de mídia da plataforma',
  'meeting-artifacts': 'Participantes e artefatos',
  'realtime-media-preview': 'Mídia ao vivo experimental',
};

const adapterStageLabels: Record<ProviderAdapterReadiness['stage'], string> = {
  ready: 'Pronto',
  'configuration-required': 'Configuração necessária',
  'admin-approval-required': 'Aprovação administrativa',
  'external-review-required': 'Revisão do provedor',
  'developer-preview': 'Developer Preview',
};

export function IntegrationSettings() {
  const router = useRouter();
  const [capabilities, setCapabilities] = useState<IntegrationCapability[]>([]);
  const [flags, setFlags] = useState<IntegrationFeatureFlags | null>(null);
  const [accounts, setAccounts] = useState<ConnectedAccount[]>([]);
  const [microsoftReadiness, setMicrosoftReadiness] = useState<MicrosoftAuthReadiness | null>(null);
  const [agentReadiness, setAgentReadiness] = useState<AgentServiceReadiness | null>(null);
  const [adapterReadiness, setAdapterReadiness] = useState<ProviderAdapterReadiness[]>([]);
  const [agentEndpoint, setAgentEndpoint] = useState('');
  const [agentToken, setAgentToken] = useState('');
  const [pairingAgent, setPairingAgent] = useState(false);
  const [connectingMicrosoft, setConnectingMicrosoft] = useState(false);
  const [outlookEvents, setOutlookEvents] = useState<OutlookCalendarEvent[]>([]);
  const [loadingEvents, setLoadingEvents] = useState(false);
  const [preparingEventId, setPreparingEventId] = useState<string | null>(null);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    try {
      const [nextCapabilities, nextFlags, nextAccounts, nextMicrosoftReadiness, nextAgentReadiness, nextAdapterReadiness] = await Promise.all([
        invoke<IntegrationCapability[]>('api_get_integration_capabilities'),
        invoke<IntegrationFeatureFlags>('api_get_integration_feature_flags'),
        invoke<ConnectedAccount[]>('api_list_connected_accounts'),
        invoke<MicrosoftAuthReadiness>('api_get_microsoft_auth_readiness'),
        invoke<AgentServiceReadiness>('api_get_agent_service_readiness'),
        invoke<ProviderAdapterReadiness[]>('api_get_meeting_provider_readiness'),
      ]);
      setCapabilities(nextCapabilities);
      setFlags(nextFlags);
      setAccounts(nextAccounts);
      setMicrosoftReadiness(nextMicrosoftReadiness);
      setAgentReadiness(nextAgentReadiness);
      setAdapterReadiness(nextAdapterReadiness);
      setAgentEndpoint(current => current || nextAgentReadiness.endpoint || '');
      setError('');
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const providers = useMemo(() => {
    const groups = new Map<IntegrationProvider, IntegrationCapability[]>();
    for (const capability of capabilities) {
      groups.set(capability.provider, [...(groups.get(capability.provider) || []), capability]);
    }
    return [...groups.entries()];
  }, [capabilities]);
  const microsoftAccount = accounts.find(account => account.provider === 'microsoft');

  useEffect(() => {
    if (!microsoftAccount) {
      setOutlookEvents([]);
      return;
    }
    const startsAt = new Date();
    const endsAt = new Date(startsAt.getTime() + 14 * 24 * 60 * 60 * 1000);
    setLoadingEvents(true);
    void invoke<OutlookCalendarEvent[]>('api_list_outlook_events', {
      accountId: microsoftAccount.id,
      startsAt: startsAt.toISOString(),
      endsAt: endsAt.toISOString(),
    }).then(setOutlookEvents).catch(reason => setError(String(reason))).finally(() => setLoadingEvents(false));
  }, [microsoftAccount]);

  const disconnect = async (account: ConnectedAccount) => {
    if (!window.confirm(`Desconectar ${account.display_name}? Os tokens serão removidos do cofre seguro; suas Notas não serão apagadas.`)) return;
    try {
      const next = await invoke<ConnectedAccount[]>('api_disconnect_integration_account', { accountId: account.id });
      setAccounts(next);
    } catch (reason) {
      setError(String(reason));
    }
  };

  const connectMicrosoft = async () => {
    setConnectingMicrosoft(true);
    setError('');
    try {
      await invoke<ConnectedAccount>('api_connect_microsoft_calendar');
      await load();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setConnectingMicrosoft(false);
    }
  };

  const prepareOutlookEvent = async (event: OutlookCalendarEvent) => {
    if (!microsoftAccount) return;
    setPreparingEventId(event.id);
    setError('');
    try {
      const prepared = await invoke<PreparedOutlookNote>('api_create_note_from_outlook_event', {
        accountId: microsoftAccount.id,
        eventId: event.id,
      });
      window.dispatchEvent(new CustomEvent('notes-changed'));
      window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      router.push(`/notes?id=${prepared.note_id}`);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setPreparingEventId(null);
    }
  };

  const pairAgent = async () => {
    setPairingAgent(true); setError('');
    try {
      const readiness = await invoke<AgentServiceReadiness>('api_pair_agent_service', { endpoint: agentEndpoint, pairingToken: agentToken });
      setAgentReadiness(readiness); setAgentToken(''); await load();
    } catch (reason) { setError(String(reason)); }
    finally { setPairingAgent(false); }
  };

  const disconnectAgent = async () => {
    if (!window.confirm('Desconectar o serviço do Agente Empathy? Sessões ativas devem ser encerradas antes no painel da Nota.')) return;
    try { await invoke('api_disconnect_agent_service'); setAgentToken(''); setAgentEndpoint(''); await load(); }
    catch (reason) { setError(String(reason)); }
  };

  return (
    <div className="integration-settings">
      <section className="integration-intro">
        <div className="integration-intro-icon"><ShieldCheck /></div>
        <div><p className="eyebrow">Controle do usuário</p><h2>Contas e reuniões</h2><p>Calendário, e-mail e presença de agentes usam consentimentos independentes. Nada é lido ou enviado enquanto a integração correspondente estiver desativada.</p></div>
      </section>

      {error && <div className="integration-error" role="alert">{error}</div>}

      {accounts.length > 0 && <section className="integration-accounts"><h3>Contas conectadas</h3>{accounts.map(account => <article key={account.id}><div><strong>{account.display_name}</strong><span>{account.email}</span><small>{account.granted_permissions.join(' · ')}</small></div><Button variant="ghost" onClick={() => void disconnect(account)}>Desconectar</Button></article>)}</section>}

      {microsoftAccount && <section className="outlook-calendar-preview"><header><div><p className="eyebrow">Próximos 14 dias</p><h3>Calendário do Outlook</h3></div><span>{loadingEvents ? 'Atualizando…' : `${outlookEvents.length} evento(s)`}</span></header>{!loadingEvents && outlookEvents.length === 0 && <p className="outlook-calendar-empty">Nenhuma reunião encontrada neste período.</p>}{outlookEvents.slice(0, 8).map(event => <article key={event.id}><time dateTime={event.starts_at}>{new Intl.DateTimeFormat('pt-BR', { weekday: 'short', day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' }).format(new Date(event.starts_at))}</time><div><strong>{event.title}</strong><span>{event.organizer?.display_name || 'Organizador não informado'} · {event.attendees.length} convidado(s)</span></div>{event.meeting_provider && <small>{event.meeting_provider === 'microsoft-teams' ? 'Teams' : event.meeting_provider === 'google-meet' ? 'Google Meet' : event.meeting_provider === 'zoom' ? 'Zoom' : 'Reunião online'}</small>}<Button size="sm" variant="ghost" disabled={preparingEventId !== null} onClick={() => void prepareOutlookEvent(event)}>{preparingEventId === event.id ? 'Preparando…' : 'Preparar'}</Button></article>)}</section>}

      <section className="agent-service-settings"><header><div><p className="eyebrow">Infraestrutura administrada</p><h3><Server /> Serviço do Agente Teams</h3><p>O desktop controla o convite e a auditoria; mídia em tempo real roda em um serviço Windows/Azure separado.</p></div><span data-ready={agentReadiness?.ready}>{agentReadiness?.ready ? 'Pronto' : agentReadiness?.reachable ? 'Configuração incompleta' : 'Desconectado'}</span></header>{agentReadiness?.configured && <div className="agent-service-current"><strong>{agentReadiness.visible_name}</strong><small>{agentReadiness.endpoint}</small>{agentReadiness.missing.length > 0 && <p>Pendente: {agentReadiness.missing.join(' · ')}</p>}{agentReadiness.service_error && <p>{agentReadiness.service_error}</p>}<Button size="sm" variant="ghost" onClick={() => void disconnectAgent()}><Unplug />Desconectar</Button></div>}<div className="agent-service-pair"><label>Endpoint HTTPS<input value={agentEndpoint} onChange={event => setAgentEndpoint(event.target.value)} placeholder="https://agent.empathy.ai" /></label><label>Token de pareamento<input type="password" autoComplete="new-password" value={agentToken} onChange={event => setAgentToken(event.target.value)} placeholder="Fornecido pelo administrador" /></label><Button disabled={!agentEndpoint || agentToken.length < 24 || pairingAgent} onClick={() => void pairAgent()}>{pairingAgent ? 'Validando serviço…' : agentReadiness?.configured ? 'Parear novamente' : 'Parear serviço'}</Button></div><small>O token fica no cofre do sistema e nunca entra nas Notas. Parear não concede consentimento de reunião.</small></section>

      <section className="meeting-adapter-readiness" aria-labelledby="meeting-adapters-title">
        <header><div><p className="eyebrow">Capacidades reais</p><h3 id="meeting-adapters-title">Como o Empathy participa em cada plataforma</h3><p>Nem toda plataforma oferece um participante de IA. Cada adaptador declara presença, dados acessados e bloqueios antes de poder ser ativado.</p></div></header>
        <div>{adapterReadiness.map((adapter, index) => <article key={`${adapter.provider}-${adapter.mode}`} data-ready={adapter.ready}>
          <div className="meeting-adapter-heading"><strong>{providerNames[adapter.provider]}</strong><span>{adapterStageLabels[adapter.stage]}</span></div>
          <p>{adapterModeLabels[adapter.mode]}</p>
          <ul aria-label="Capacidades do adaptador">
            <li data-enabled={adapter.reads_participants}>Participantes</li>
            <li data-enabled={adapter.reads_artifacts}>Artefatos</li>
            <li data-enabled={adapter.streams_realtime_media}>Mídia ao vivo</li>
          </ul>
          <small>{adapter.privacy_note}</small>
          {adapter.missing.length > 0 && <details><summary>{adapter.missing.length} requisito(s) pendente(s)</summary><ul>{adapter.missing.map(item => <li key={item}>{item}</li>)}</ul></details>}
          <a href={adapter.documentation_url} target="_blank" rel="noreferrer">Documentação oficial <ExternalLink aria-hidden="true" /></a>
          {index === 0 && adapter.ready && <span className="sr-only">O adaptador de participante visível está pronto.</span>}
        </article>)}</div>
      </section>

      <div className="integration-provider-list">
        {providers.map(([provider, items]) => <section className="integration-provider" key={provider}>
          <header><div><h3>{providerNames[provider]}</h3><p>{provider === 'microsoft' ? 'A conta será conectada primeiro ao calendário; e-mails exigirão uma segunda autorização.' : 'A disponibilidade depende das políticas e aprovações da plataforma.'}</p></div>{provider === 'microsoft' && accounts.every(account => account.provider !== 'microsoft') && <Button disabled={!microsoftReadiness?.configured || connectingMicrosoft} onClick={() => void connectMicrosoft()}>{connectingMicrosoft ? 'Aguardando navegador…' : microsoftReadiness?.configured ? 'Conectar calendário' : 'Client ID necessário'}</Button>}</header>
          <div>{items.map(capability => {
            const Icon = capabilityIcons[capability.id];
            const enabled = Boolean(flags?.[capability.id]);
            return <article className="integration-capability" key={capability.id} data-enabled={enabled}>
              <Icon />
              <div><strong>{capability.name}</strong><p>{capability.description}</p><small>{capability.prerequisites.join(' · ')}</small></div>
              <span>{enabled ? 'Ativo' : stageLabels[capability.stage]}</span>
            </article>;
          })}</div>
        </section>)}
      </div>

      <p className="integration-footnote"><ExternalLink /> Os conectores serão ativados somente depois que as credenciais públicas e aprovações necessárias estiverem configuradas. Tokens nunca serão gravados nas Notas Markdown.</p>
    </div>
  );
}

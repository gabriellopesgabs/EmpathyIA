'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CalendarDays, ExternalLink, Mail, ShieldCheck, UsersRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { ConnectedAccount, IntegrationCapability, IntegrationFeatureFlags, IntegrationProvider } from '@/types/integrations';

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

export function IntegrationSettings() {
  const [capabilities, setCapabilities] = useState<IntegrationCapability[]>([]);
  const [flags, setFlags] = useState<IntegrationFeatureFlags | null>(null);
  const [accounts, setAccounts] = useState<ConnectedAccount[]>([]);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    try {
      const [nextCapabilities, nextFlags, nextAccounts] = await Promise.all([
        invoke<IntegrationCapability[]>('api_get_integration_capabilities'),
        invoke<IntegrationFeatureFlags>('api_get_integration_feature_flags'),
        invoke<ConnectedAccount[]>('api_list_connected_accounts'),
      ]);
      setCapabilities(nextCapabilities);
      setFlags(nextFlags);
      setAccounts(nextAccounts);
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

  const disconnect = async (account: ConnectedAccount) => {
    if (!window.confirm(`Desconectar ${account.display_name}? Os tokens serão removidos do cofre seguro; suas Notas não serão apagadas.`)) return;
    try {
      const next = await invoke<ConnectedAccount[]>('api_disconnect_integration_account', { accountId: account.id });
      setAccounts(next);
    } catch (reason) {
      setError(String(reason));
    }
  };

  return (
    <div className="integration-settings">
      <section className="integration-intro">
        <div className="integration-intro-icon"><ShieldCheck /></div>
        <div><p className="eyebrow">Controle do usuário</p><h2>Contas e reuniões</h2><p>Calendário, e-mail e presença de agentes usam consentimentos independentes. Nada é lido ou enviado enquanto a integração correspondente estiver desativada.</p></div>
      </section>

      {error && <div className="integration-error" role="alert">{error}</div>}

      {accounts.length > 0 && <section className="integration-accounts"><h3>Contas conectadas</h3>{accounts.map(account => <article key={account.id}><div><strong>{account.display_name}</strong><span>{account.email}</span><small>{account.granted_permissions.join(' · ')}</small></div><Button variant="ghost" onClick={() => void disconnect(account)}>Desconectar</Button></article>)}</section>}

      <div className="integration-provider-list">
        {providers.map(([provider, items]) => <section className="integration-provider" key={provider}>
          <header><div><h3>{providerNames[provider]}</h3><p>{provider === 'microsoft' ? 'A conta será conectada primeiro ao calendário; e-mails exigirão uma segunda autorização.' : 'A disponibilidade depende das políticas e aprovações da plataforma.'}</p></div></header>
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

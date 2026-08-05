'use client';

import { useEffect, useState } from 'react';
import { AudioWaveform, Link2, Mic, Settings2, Sparkles } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { TranscriptSettings } from '@/components/TranscriptSettings';
import { RecordingSettings } from '@/components/RecordingSettings';
import { PreferenceSettings } from '@/components/PreferenceSettings';
import { SummaryModelSettings } from '@/components/SummaryModelSettings';
import { IntegrationSettings } from '@/components/IntegrationSettings';
import { useConfig } from '@/contexts/ConfigContext';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

const TABS = [
  { value: 'general', label: 'Geral', description: 'Notificações, arquivos e privacidade', icon: Settings2 },
  { value: 'recording', label: 'Gravação', description: 'Áudio, dispositivos e salvamento', icon: Mic },
  { value: 'transcriptionModels', label: 'Transcrição', description: 'Idioma e reconhecimento de fala', icon: AudioWaveform },
  { value: 'summaryModels', label: 'Inteligência', description: 'Resumos e modelos locais', icon: Sparkles },
  { value: 'integrations', label: 'Integrações', description: 'Outlook e agentes de reunião', icon: Link2 },
] as const;

type SettingsTab = typeof TABS[number]['value'];

function tabFromLocation(): SettingsTab {
  if (typeof window === 'undefined') return 'general';
  const value = new URLSearchParams(window.location.search).get('tab');
  return TABS.some(tab => tab.value === value) ? value as SettingsTab : 'general';
}

export default function SettingsPage() {
  const router = useRouter();
  const { transcriptModelConfig, setTranscriptModelConfig } = useConfig();
  const currentTranscriptProvider = transcriptModelConfig.provider;
  const [activeTab, setActiveTab] = useState<SettingsTab>(tabFromLocation);

  useEffect(() => {
    void invoke<Record<string, unknown> | null>('api_get_transcript_config').then(config => {
      if (!config) return;
      setTranscriptModelConfig({
        provider: (config.provider as typeof currentTranscriptProvider) || 'parakeet',
        model: (config.model as string) || 'parakeet-tdt-0.6b-v3-int8',
        apiKey: (config.apiKey as string | null) || null,
      });
    }).catch(error => console.warn('Não foi possível carregar a configuração de transcrição:', error));
  }, [currentTranscriptProvider, setTranscriptModelConfig]);

  const changeTab = (value: string) => {
    const next = TABS.some(tab => tab.value === value) ? value as SettingsTab : 'general';
    setActiveTab(next);
    router.replace(`/settings?tab=${next}`, { scroll: false });
  };

  return (
    <div className="settings-shell">
      <header className="settings-titlebar"><div><p className="eyebrow">Empathy</p><h1>Configurações</h1></div></header>
      <Tabs value={activeTab} onValueChange={changeTab} className="settings-layout">
        <TabsList className="settings-sidebar" aria-label="Categorias de configuração">
          {TABS.map(tab => {
            const Icon = tab.icon;
            return <TabsTrigger key={tab.value} value={tab.value} className="settings-nav-item"><Icon /><span><strong>{tab.label}</strong><small>{tab.description}</small></span></TabsTrigger>;
          })}
        </TabsList>
        <div className="settings-content">
          <TabsContent value="general"><PreferenceSettings /></TabsContent>
          <TabsContent value="recording"><RecordingSettings /></TabsContent>
          <TabsContent value="transcriptionModels"><TranscriptSettings transcriptModelConfig={transcriptModelConfig} setTranscriptModelConfig={setTranscriptModelConfig} /></TabsContent>
          <TabsContent value="summaryModels"><SummaryModelSettings /></TabsContent>
          <TabsContent value="integrations"><IntegrationSettings /></TabsContent>
        </div>
      </Tabs>
    </div>
  );
}

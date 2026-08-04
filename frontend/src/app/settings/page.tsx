'use client';

import React, { useState, useEffect, useLayoutEffect, useRef } from 'react';
import { ArrowLeft, Settings2, Mic, AudioWaveform, SparkleIcon } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { TranscriptSettings } from '@/components/TranscriptSettings';
import { RecordingSettings } from '@/components/RecordingSettings';
import { PreferenceSettings } from '@/components/PreferenceSettings';
import { SummaryModelSettings } from '@/components/SummaryModelSettings';
import { useConfig } from '@/contexts/ConfigContext';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

// Tabs configuration (constant)
const TABS = [
  { value: 'general', label: 'General', icon: Settings2 },
  { value: 'recording', label: 'Recordings', icon: Mic },
  { value: 'transcriptionModels', label: 'Speech & Voice', icon: AudioWaveform },
  { value: 'summaryModels', label: 'AI Models & Agents', icon: SparkleIcon },
] as const;

type SettingsTab = typeof TABS[number]['value'];

function tabFromLocation(): SettingsTab {
  if (typeof window === 'undefined') return 'general';
  const requested = new URLSearchParams(window.location.search).get('tab');
  return TABS.some(tab => tab.value === requested) ? requested as SettingsTab : 'general';
}

export default function SettingsPage() {
  const router = useRouter();
  const { transcriptModelConfig, setTranscriptModelConfig } = useConfig();

  // Animation state for tabs
  const [activeTab, setActiveTab] = useState<SettingsTab>(tabFromLocation);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [underlineStyle, setUnderlineStyle] = useState({ left: 0, width: 0 });

  // Load saved transcript configuration on mount
  useEffect(() => {
    const loadTranscriptConfig = async () => {
      try {
        const config = await invoke('api_get_transcript_config') as any;
        if (config) {
          console.log('Loaded saved transcript config:', config);
          setTranscriptModelConfig({
            provider: config.provider || 'localWhisper',
            model: config.model || 'large-v3',
            apiKey: config.apiKey || null
          });
        }
      } catch (error) {
        console.error('Failed to load transcript config:', error);
      }
    };
    loadTranscriptConfig();
  }, [setTranscriptModelConfig]);

  // Update underline position when active tab changes
  useLayoutEffect(() => {
    const activeIndex = TABS.findIndex(tab => tab.value === activeTab);
    const activeTabElement = tabRefs.current[activeIndex];

    if (activeTabElement) {
      const { offsetLeft, offsetWidth } = activeTabElement;
      setUnderlineStyle({ left: offsetLeft, width: offsetWidth });
    }
  }, [activeTab]);

  const changeTab = (value: string) => {
    const next = TABS.some(tab => tab.value === value) ? value as SettingsTab : 'general';
    setActiveTab(next);
    router.replace(`/settings?tab=${next}`, { scroll: false });
  };

  return (
    <div className="h-screen bg-gray-50 flex flex-col dark:bg-gray-950">
      {/* Fixed Header */}
      <div className="sticky top-0 z-10 bg-gray-50 border-b border-gray-200 dark:bg-gray-950 dark:border-gray-800">
        <div className="max-w-6xl mx-auto px-4 py-5 sm:px-8 sm:py-6">
          <div className="flex items-center gap-4">
            <button
              onClick={() => router.back()}
              className="flex items-center gap-2 text-gray-600 hover:text-gray-900 transition-colors"
            >
              <ArrowLeft className="w-5 h-5" />
              <span>Back</span>
            </button>
            <h1 className="text-2xl font-bold sm:text-3xl">Configurações</h1>
          </div>
        </div>
      </div>

      {/* Scrollable Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-6xl mx-auto p-4 pt-5 sm:p-8 sm:pt-6">
          {/* Tabs */}
          <Tabs value={activeTab} onValueChange={changeTab}>
            <div className="overflow-x-auto">
            <TabsList className="bg-transparent relative min-w-max rounded-none border-b border-gray-200 p-0 h-auto dark:border-gray-800">
              {TABS.map((tab, index) => {
                const Icon = tab.icon;
                return (
                  <TabsTrigger
                    key={tab.value}
                    value={tab.value}
                    ref={el => { tabRefs.current[index] = el }}
                    className="flex items-center gap-2 px-6 py-4 bg-transparent rounded-none border-0 data-[state=active]:bg-transparent data-[state=active]:text-blue-600 data-[state=active]:shadow-none text-gray-600 hover:text-gray-900 relative z-10"
                  >
                    <Icon className="w-4 h-4" />
                    {tab.label}
                  </TabsTrigger>
                );
              })}

              <motion.div
                className="absolute bottom-0 z-20 h-0.5 bg-blue-600"
                layoutId="underline"
                style={{ left: underlineStyle.left, width: underlineStyle.width }}
                transition={{ type: 'spring', stiffness: 400, damping: 40 }}
              />
            </TabsList>
            </div>

            <TabsContent value="general">
              <PreferenceSettings />
            </TabsContent>
            <TabsContent value="recording">
              <RecordingSettings />
            </TabsContent>
            <TabsContent value="transcriptionModels">
              <TranscriptSettings
                transcriptModelConfig={transcriptModelConfig}
                setTranscriptModelConfig={setTranscriptModelConfig}
              />
            </TabsContent>
            <TabsContent value="summaryModels">
              <SummaryModelSettings />
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </div>
  );
};

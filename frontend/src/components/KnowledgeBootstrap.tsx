'use client';

import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/** Keeps the disposable knowledge index synchronized with user-owned Markdown. */
export function KnowledgeBootstrap() {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    const reindex = async () => {
      try {
        await invoke('api_reindex_knowledge');
        if (!cancelled) window.dispatchEvent(new CustomEvent('knowledge-index-updated'));
      } catch (error) {
        console.warn('Knowledge index could not be refreshed:', error);
      }
    };

    const initialize = async () => {
      await reindex();
      if (cancelled) return;
      try {
        await invoke('api_start_knowledge_watcher');
        unlisten = await listen('knowledge-files-changed', () => {
          if (timer) clearTimeout(timer);
          timer = setTimeout(reindex, 800);
        });
      } catch (error) {
        console.warn('Knowledge file watcher is unavailable:', error);
      }
    };
    initialize();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      unlisten?.();
    };
  }, []);

  return null;
}

'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChevronDown, Loader2, Network } from 'lucide-react';
import { KnowledgeGraphView } from '@/components/KnowledgeGraph';
import type { KnowledgeGraph } from '@/lib/knowledgeGraph';

export function MeetingKnowledgeGraph({ meetingId }: { meetingId: string }) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [graph, setGraph] = useState<KnowledgeGraph>({ nodes: [], edges: [], truncated: false });

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setGraph(await invoke<KnowledgeGraph>('api_get_knowledge_graph', { meetingId }));
    } finally {
      setLoading(false);
    }
  }, [meetingId]);

  useEffect(() => {
    if (!open) return;
    load();
    const refresh = () => load();
    window.addEventListener('knowledge-index-updated', refresh);
    return () => window.removeEventListener('knowledge-index-updated', refresh);
  }, [load, open]);

  return (
    <section className="mx-6 mb-5 rounded-xl border bg-white dark:bg-gray-900">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(value => !value)}
        className="flex w-full items-center justify-between gap-3 p-4 text-left"
      >
        <span className="flex items-center gap-2 font-semibold"><Network className="h-4 w-4 text-blue-600" /> Grafo desta reunião</span>
        <ChevronDown className={`h-4 w-4 transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>
      {open && (
        <div className="border-t p-3">
          {loading ? (
            <div className="flex min-h-48 items-center justify-center"><Loader2 className="animate-spin text-gray-400" /></div>
          ) : (
            <KnowledgeGraphView
              graph={graph}
              title="Contexto da reunião"
              subtitle="Documentos, pessoas, projeto, tags, tarefas e decisões vinculados a esta conversa."
            />
          )}
        </div>
      )}
    </section>
  );
}

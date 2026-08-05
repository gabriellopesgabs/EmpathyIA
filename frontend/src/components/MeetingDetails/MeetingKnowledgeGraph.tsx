'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Loader2, RefreshCw } from 'lucide-react';
import { KnowledgeGraphView } from '@/components/KnowledgeGraph';
import { mergeMeetingKnowledgeGraphs, type KnowledgeGraph } from '@/lib/knowledgeGraph';
import { Button } from '@/components/ui/button';

export function MeetingKnowledgeGraph({
  meetingId,
  fallbackGraph,
}: {
  meetingId: string;
  fallbackGraph: KnowledgeGraph;
}) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [graph, setGraph] = useState<KnowledgeGraph>({ nodes: [], edges: [], truncated: false });
  const visibleGraph = useMemo(
    () => mergeMeetingKnowledgeGraphs(graph, fallbackGraph),
    [fallbackGraph, graph],
  );

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setGraph(await invoke<KnowledgeGraph>('api_get_knowledge_graph', { meetingId }));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, [meetingId]);

  useEffect(() => {
    void load();
    const refresh = () => void load();
    window.addEventListener('knowledge-index-updated', refresh);
    return () => window.removeEventListener('knowledge-index-updated', refresh);
  }, [load]);

  return (
    <section className="relative min-h-full rounded-xl bg-slate-50 p-3 dark:bg-gray-950 sm:p-5">
      <div className="absolute right-6 top-6 z-10">
        <Button size="sm" variant="outline" onClick={() => void load()} disabled={loading}>
          {loading ? <Loader2 className="animate-spin" /> : <RefreshCw />}
          <span className="hidden sm:inline">Atualizar</span>
        </Button>
      </div>
      <KnowledgeGraphView
        graph={visibleGraph}
        title="Grafo da nota"
        subtitle={error
          ? 'Exibindo os temas derivados da transcrição salva; o índice completo será combinado quando estiver disponível.'
          : 'Transcrição, resumo, pessoas, projetos, tags, tarefas, decisões e temas desta nota.'}
      />
      {error && <p className="mt-2 px-1 text-xs text-amber-700">O índice completo não pôde ser carregado: {error}</p>}
    </section>
  );
}

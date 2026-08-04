import { describe, expect, it } from 'vitest';
import { buildLiveKnowledgeGraph } from '@/lib/knowledgeGraph';

describe('live knowledge graph', () => {
  it('extracts stable topics and recent transcript segments', () => {
    const graph = buildLiveKnowledgeGraph([
      { id: '1', text: 'Precisamos revisar o lançamento do produto', timestamp: '10:00' },
      { id: '2', text: 'O produto precisa de revisão antes do lançamento', timestamp: '10:01' },
    ], 'Produto');
    expect(graph.nodes[0]).toMatchObject({ id: 'live-meeting', label: 'Produto' });
    expect(graph.nodes.some(node => node.id === 'topic:produto' && node.count === 2)).toBe(true);
    expect(graph.nodes.filter(node => node.kind === 'segment')).toHaveLength(2);
    expect(graph.edges.some(edge => edge.kind === 'mentions')).toBe(true);
  });

  it('does not index partial or empty transcript fragments', () => {
    const graph = buildLiveKnowledgeGraph([
      { id: 'partial', text: 'rascunho temporário', timestamp: '10:00', is_partial: true },
      { id: 'empty', text: '   ', timestamp: '10:01' },
    ]);
    expect(graph.nodes).toHaveLength(1);
    expect(graph.edges).toHaveLength(0);
  });
});

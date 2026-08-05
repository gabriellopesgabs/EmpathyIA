import { describe, expect, it } from 'vitest';
import {
  buildLiveKnowledgeGraph,
  buildMarkdownKnowledgeGraph,
  mergeMeetingKnowledgeGraphs,
} from '@/lib/knowledgeGraph';

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

  it('uses partial live fragments but ignores empty transcript fragments', () => {
    const graph = buildLiveKnowledgeGraph([
      { id: 'partial', text: 'rascunho temporário', timestamp: '10:00', is_partial: true },
      { id: 'empty', text: '   ', timestamp: '10:01' },
    ]);
    expect(graph.nodes.some(node => node.id === 'segment:partial' && node.partial)).toBe(true);
    expect(graph.nodes.some(node => node.id === 'segment:empty')).toBe(false);
    expect(graph.edges.some(edge => edge.target === 'segment:partial')).toBe(true);
  });

  it('builds topics from a written Markdown note', () => {
    const graph = buildMarkdownKnowledgeGraph(
      '# Planejamento\n\n## Produto\nPrecisamos revisar o produto e o lançamento.\n\n- [ ] Preparar lançamento',
      'Planejamento',
    );
    expect(graph.nodes[0]).toMatchObject({ kind: 'meeting', label: 'Planejamento' });
    expect(graph.nodes.some(node => node.id === 'topic:lançamento')).toBe(true);
    expect(graph.nodes.some(node => node.kind === 'segment')).toBe(true);
  });

  it('attaches saved transcript topics to the indexed meeting node', () => {
    const indexed = {
      nodes: [{ id: 'meeting:file', label: 'Reunião', kind: 'meeting', count: 1 }],
      edges: [],
      truncated: false,
    };
    const semantic = buildLiveKnowledgeGraph([
      { id: '1', text: 'produto lançamento', timestamp: '10:00' },
    ], 'Reunião');
    const merged = mergeMeetingKnowledgeGraphs(indexed, semantic);
    expect(merged.nodes.filter(node => node.kind === 'meeting')).toHaveLength(1);
    expect(merged.edges.some(edge => edge.source === 'meeting:file' && edge.kind === 'topic')).toBe(true);
  });

  it('indexes skill results and the three augmented-intelligence relationships', () => {
    const graph = buildMarkdownKnowledgeGraph(`Texto humano

<!-- empathy-skill-result
id: result-1
skill_id: connect-memory
skill_name: Conectar com a memória
layer: collective
created_at: 2026-08-05T00:00:00Z
source_scope: note
provider: builtin-ai
model: qwen
context_documents: ["note-2"]
-->
## Conexões

*Skill (Conectar com a memória)*

Uma conexão.
<!-- /empathy-skill-result -->`, 'Nota');
    expect(graph.nodes.some(node => node.kind === 'collective')).toBe(true);
    expect(graph.edges.map(edge => edge.kind)).toEqual(expect.arrayContaining(['contains', 'generated_by', 'used_context']));
  });
});

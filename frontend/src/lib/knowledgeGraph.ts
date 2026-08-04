import type { Transcript } from '@/types';

export type KnowledgeGraphNodeKind =
  | 'meeting' | 'transcript' | 'summary' | 'note'
  | 'project' | 'person' | 'tag' | 'task' | 'decision'
  | 'topic' | 'segment';

export type KnowledgeGraphNode = {
  id: string;
  label: string;
  kind: KnowledgeGraphNodeKind | string;
  meeting_id?: string;
  path?: string;
  count: number;
  partial?: boolean;
};

export type KnowledgeGraphEdge = {
  id: string;
  source: string;
  target: string;
  kind: string;
  weight: number;
};

export type KnowledgeGraph = {
  nodes: KnowledgeGraphNode[];
  edges: KnowledgeGraphEdge[];
  truncated: boolean;
};

const STOP_WORDS = new Set([
  'ainda', 'assim', 'aqui', 'cada', 'como', 'com', 'contra', 'depois', 'desde',
  'dessa', 'desse', 'desta', 'deste', 'disso', 'então', 'entre', 'essa', 'esse',
  'esta', 'este', 'está', 'fazer', 'feito', 'gente', 'isso', 'mais', 'mas', 'mesmo',
  'muito', 'não', 'nossa', 'nosso', 'onde', 'para', 'pela', 'pelo', 'pode', 'porque',
  'qual', 'quando', 'ser', 'sobre', 'também', 'tem', 'ter', 'tudo', 'uma', 'vamos',
  'você', 'vocês', 'agora', 'alguma', 'algum', 'aquela', 'aquele', 'foram', 'seria',
  'the', 'and', 'that', 'this', 'with', 'from', 'have', 'will', 'would', 'about',
  'there', 'their', 'they', 'what', 'when', 'where', 'which', 'your', 'just', 'into',
]);

function words(text: string): string[] {
  return (text.toLocaleLowerCase('pt-BR').match(/[\p{L}\p{N}][\p{L}\p{N}-]{2,}/gu) ?? [])
    .filter(word => word.length >= 4 && !STOP_WORDS.has(word) && !/^\d+$/.test(word));
}

function edgeId(source: string, target: string, kind: string): string {
  return `${kind}:${source}:${target}`;
}

export function buildLiveKnowledgeGraph(
  transcripts: Transcript[],
  meetingTitle = 'Reunião ao vivo',
): KnowledgeGraph {
  // Short Whisper chunks are marked as partial while recording. They are still
  // the best available live evidence and must feed both the transcript and graph.
  const liveTranscripts = transcripts.filter(item => item.text.trim());
  const frequencies = new Map<string, number>();
  const segmentWords = new Map<string, Set<string>>();

  for (const transcript of liveTranscripts) {
    const unique = new Set(words(transcript.text));
    segmentWords.set(transcript.id, unique);
    unique.forEach(word => frequencies.set(word, (frequencies.get(word) ?? 0) + 1));
  }

  const topics = [...frequencies.entries()]
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 12);
  const topicSet = new Set(topics.map(([topic]) => topic));
  const nodes: KnowledgeGraphNode[] = [{
    id: 'live-meeting', label: meetingTitle || 'Reunião ao vivo', kind: 'meeting', count: liveTranscripts.length,
  }];
  const edges: KnowledgeGraphEdge[] = [];

  for (const [topic, count] of topics) {
    const id = `topic:${topic}`;
    nodes.push({ id, label: topic, kind: 'topic', count });
    edges.push({ id: edgeId('live-meeting', id, 'topic'), source: 'live-meeting', target: id, kind: 'topic', weight: count });
  }

  for (const transcript of liveTranscripts.slice(-6)) {
    const id = `segment:${transcript.id}`;
    const label = transcript.text.length > 72 ? `${transcript.text.slice(0, 69)}…` : transcript.text;
    nodes.push({ id, label, kind: 'segment', count: 1, partial: transcript.is_partial });
    edges.push({ id: edgeId('live-meeting', id, 'segment'), source: 'live-meeting', target: id, kind: 'segment', weight: 1 });
    for (const topic of segmentWords.get(transcript.id) ?? []) {
      if (!topicSet.has(topic)) continue;
      edges.push({
        id: edgeId(id, `topic:${topic}`, 'mentions'), source: id, target: `topic:${topic}`, kind: 'mentions', weight: 1,
      });
    }
  }

  return { nodes, edges, truncated: false };
}

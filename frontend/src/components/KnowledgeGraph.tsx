'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Maximize2, Minus, Plus, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { KnowledgeGraph, KnowledgeGraphNode } from '@/lib/knowledgeGraph';

type Point = KnowledgeGraphNode & { x: number; y: number; radius: number; degree: number };

const COLORS: Record<string, string> = {
  meeting: '#2563eb', transcript: '#0ea5e9', summary: '#8b5cf6', note: '#64748b',
  project: '#f59e0b', person: '#10b981', tag: '#ec4899', task: '#ef4444',
  decision: '#7c3aed', topic: '#0891b2', segment: '#94a3b8',
};

const KIND_LABELS: Record<string, string> = {
  meeting: 'Reunião', transcript: 'Transcrição', summary: 'Resumo', note: 'Nota',
  project: 'Projeto', person: 'Pessoa', tag: 'Tag', task: 'Tarefa', decision: 'Decisão',
  topic: 'Tema', segment: 'Trecho recente',
};

function hash(value: string): number {
  let result = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    result ^= value.charCodeAt(index);
    result = Math.imul(result, 16777619);
  }
  return result >>> 0;
}

function prepareGraph(graph: KnowledgeGraph, width: number, height: number): { nodes: Point[]; edges: KnowledgeGraph['edges'] } {
  const degree = new Map<string, number>();
  graph.edges.forEach(edge => {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + edge.weight);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + edge.weight);
  });
  const selected = [...graph.nodes]
    .sort((left, right) => (degree.get(right.id) ?? 0) - (degree.get(left.id) ?? 0) || left.id.localeCompare(right.id))
    .slice(0, 180);
  const visible = new Set(selected.map(node => node.id));
  const centerX = width / 2;
  const centerY = height / 2;
  const ring = Math.max(70, Math.min(width, height) * 0.36);
  const primary = selected.filter(node => ['meeting', 'transcript', 'summary', 'note'].includes(node.kind));
  const semantic = selected.filter(node => !['meeting', 'transcript', 'summary', 'note'].includes(node.kind));

  const place = (node: KnowledgeGraphNode, index: number, total: number, radius: number): Point => {
    const stableOffset = (hash(node.id) % 1000) / 1000 * Math.PI * 2;
    const angle = stableOffset + (index / Math.max(total, 1)) * Math.PI * 2;
    const nodeDegree = degree.get(node.id) ?? 1;
    return {
      ...node,
      degree: nodeDegree,
      radius: Math.min(14, 5 + Math.sqrt(Math.max(node.count, nodeDegree))),
      x: centerX + Math.cos(angle) * radius,
      y: centerY + Math.sin(angle) * radius,
    };
  };

  const nodes = [
    ...primary.map((node, index) => primary.length === 1
      ? { ...place(node, 0, 1, 0), x: centerX, y: centerY }
      : place(node, index, primary.length, ring * 0.48)),
    ...semantic.map((node, index) => place(node, index, semantic.length, ring)),
  ];
  return { nodes, edges: graph.edges.filter(edge => visible.has(edge.source) && visible.has(edge.target)) };
}

export function KnowledgeGraphView({
  graph, title, subtitle, live = false, onOpenNode, className = '',
}: {
  graph: KnowledgeGraph;
  title: string;
  subtitle?: string;
  live?: boolean;
  onOpenNode?: (node: KnowledgeGraphNode) => void;
  className?: string;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ x: number; y: number; panX: number; panY: number; moved: boolean } | null>(null);
  const [size, setSize] = useState({ width: 720, height: 440 });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [hiddenKinds, setHiddenKinds] = useState<Set<string>>(() => new Set());
  const kinds = useMemo(() => [...new Set(graph.nodes.map(node => node.kind))].sort(), [graph.nodes]);
  const visibleGraph = useMemo(() => ({
    ...graph,
    nodes: graph.nodes.filter(node => !hiddenKinds.has(node.kind)),
  }), [graph, hiddenKinds]);
  const prepared = useMemo(() => prepareGraph(visibleGraph, size.width, size.height), [size, visibleGraph]);
  const selected = prepared.nodes.find(node => node.id === selectedId) ?? null;

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const observer = new ResizeObserver(entries => {
      const width = Math.max(280, Math.floor(entries[0].contentRect.width));
      const height = Math.max(320, Math.min(560, Math.floor(width * 0.62)));
      setSize(current => current.width === width && current.height === height ? current : { width, height });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const screenPoint = useCallback((node: Point) => ({
    x: size.width / 2 + (node.x - size.width / 2) * zoom + pan.x,
    y: size.height / 2 + (node.y - size.height / 2) * zoom + pan.y,
  }), [pan, size, zoom]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.floor(size.width * ratio);
    canvas.height = Math.floor(size.height * ratio);
    canvas.style.width = `${size.width}px`;
    canvas.style.height = `${size.height}px`;
    const context = canvas.getContext('2d');
    if (!context) return;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, size.width, size.height);
    context.fillStyle = '#f8fafc';
    context.fillRect(0, 0, size.width, size.height);

    const byId = new Map(prepared.nodes.map(node => [node.id, node]));
    context.lineCap = 'round';
    prepared.edges.forEach(edge => {
      const source = byId.get(edge.source);
      const target = byId.get(edge.target);
      if (!source || !target) return;
      const start = screenPoint(source);
      const end = screenPoint(target);
      context.beginPath();
      context.moveTo(start.x, start.y);
      context.lineTo(end.x, end.y);
      context.strokeStyle = edge.kind === 'link' ? '#6366f1' : '#cbd5e1';
      context.globalAlpha = selectedId && source.id !== selectedId && target.id !== selectedId ? 0.16 : 0.55;
      context.lineWidth = Math.min(3, 0.7 + Math.sqrt(edge.weight) * 0.45);
      context.stroke();
    });
    context.globalAlpha = 1;

    const labelNodes = [...prepared.nodes]
      .sort((left, right) => right.degree - left.degree)
      .slice(0, size.width < 520 ? 7 : 14);
    const labels = new Set(labelNodes.map(node => node.id));
    prepared.nodes.forEach(node => {
      const point = screenPoint(node);
      const focused = node.id === selectedId;
      context.beginPath();
      context.arc(point.x, point.y, Math.max(4, node.radius * zoom), 0, Math.PI * 2);
      context.fillStyle = COLORS[node.kind] ?? '#64748b';
      context.globalAlpha = selectedId && !focused ? 0.58 : 0.94;
      context.fill();
      if (focused) {
        context.strokeStyle = '#0f172a';
        context.lineWidth = 3;
        context.stroke();
      }
      if (labels.has(node.id) || focused) {
        context.globalAlpha = 1;
        context.font = `${focused ? 600 : 500} 12px ui-sans-serif, system-ui`;
        context.fillStyle = '#0f172a';
        context.textAlign = 'center';
        const label = node.label.length > 30 ? `${node.label.slice(0, 27)}…` : node.label;
        context.fillText(label, point.x, point.y + Math.max(14, node.radius * zoom + 13));
      }
    });
    context.globalAlpha = 1;
  }, [prepared, screenPoint, selectedId, size, zoom]);

  const selectAt = (x: number, y: number) => {
    let nearest: Point | undefined;
    let distance = 24;
    for (const node of prepared.nodes) {
      const point = screenPoint(node);
      const candidate = Math.hypot(point.x - x, point.y - y);
      if (candidate < distance) {
        distance = candidate;
        nearest = node;
      }
    }
    setSelectedId(nearest?.id ?? null);
  };

  const reset = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
    setSelectedId(null);
    setHiddenKinds(new Set());
  };

  return (
    <section className={`overflow-hidden rounded-xl border bg-white shadow-sm dark:bg-gray-900 ${className}`}>
      <div className="flex flex-wrap items-start justify-between gap-3 border-b p-4">
        <div>
          <h2 className="flex items-center gap-2 font-semibold">{title}{live && <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-xs text-emerald-700">Ao vivo</span>}</h2>
          {subtitle && <p className="mt-1 text-sm text-gray-500">{subtitle}</p>}
        </div>
        <div className="flex gap-1" aria-label="Controles do grafo">
          <Button size="icon" variant="outline" aria-label="Diminuir zoom" onClick={() => setZoom(value => Math.max(0.55, value - 0.15))}><Minus /></Button>
          <Button size="icon" variant="outline" aria-label="Restaurar visualização" onClick={reset}><RotateCcw /></Button>
          <Button size="icon" variant="outline" aria-label="Aumentar zoom" onClick={() => setZoom(value => Math.min(2.2, value + 0.15))}><Plus /></Button>
        </div>
      </div>
      <div ref={containerRef} className="relative w-full touch-none">
        <canvas
          ref={canvasRef}
          role="img"
          aria-label={`${title}: ${visibleGraph.nodes.length} nós visíveis e ${prepared.edges.length} conexões. Use a lista abaixo para explorar com teclado.`}
          onPointerDown={event => {
            event.currentTarget.setPointerCapture(event.pointerId);
            dragRef.current = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y, moved: false };
          }}
          onPointerMove={event => {
            if (!dragRef.current) return;
            const dx = event.clientX - dragRef.current.x;
            const dy = event.clientY - dragRef.current.y;
            if (Math.abs(dx) + Math.abs(dy) > 5) dragRef.current.moved = true;
            setPan({ x: dragRef.current.panX + dx, y: dragRef.current.panY + dy });
          }}
          onPointerUp={event => {
            const drag = dragRef.current;
            dragRef.current = null;
            if (!drag?.moved) {
              const bounds = event.currentTarget.getBoundingClientRect();
              selectAt(event.clientX - bounds.left, event.clientY - bounds.top);
            }
          }}
          onPointerCancel={() => { dragRef.current = null; }}
        />
        {visibleGraph.nodes.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center p-8 text-center text-sm text-gray-500">Nenhuma conexão disponível. Reindexe o workspace ou inicie uma transcrição.</div>
        )}
      </div>
      <div className="border-t p-4">
        <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-gray-500">
          {kinds.map(kind => (
            <button
              key={kind}
              type="button"
              aria-pressed={!hiddenKinds.has(kind)}
              onClick={() => setHiddenKinds(current => {
                const next = new Set(current);
                if (next.has(kind)) next.delete(kind); else next.add(kind);
                return next;
              })}
              className={`inline-flex items-center gap-1.5 rounded-full px-1.5 py-0.5 hover:bg-gray-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-600 dark:hover:bg-gray-800 ${hiddenKinds.has(kind) ? 'opacity-35 line-through' : ''}`}
            >
              <span className="h-2.5 w-2.5 rounded-full" style={{ background: COLORS[kind] ?? '#64748b' }} />{KIND_LABELS[kind] ?? kind}
            </button>
          ))}
        </div>
        {graph.truncated && <p className="mt-2 text-xs text-amber-700">Visão resumida para preservar desempenho. Use busca e filtros para aprofundar.</p>}
        {selected && (
          <div className="mt-3 flex items-start justify-between gap-3 rounded-lg bg-slate-50 p-3 dark:bg-gray-800">
            <div><p className="text-xs text-gray-500">{KIND_LABELS[selected.kind] ?? selected.kind}</p><p className="text-sm font-medium">{selected.label}</p><p className="text-xs text-gray-400">{selected.degree} conexões</p></div>
            {onOpenNode && (selected.meeting_id || selected.path) && <Button size="sm" variant="outline" onClick={() => onOpenNode(selected)}><Maximize2 /> Abrir</Button>}
          </div>
        )}
        <details className="mt-3">
          <summary className="cursor-pointer text-xs font-medium text-gray-500">Explorar por lista</summary>
          <div className="mt-2 flex max-h-32 flex-wrap gap-2 overflow-auto">
            {[...prepared.nodes].sort((a, b) => b.degree - a.degree).map(node => (
              <button key={node.id} onClick={() => setSelectedId(node.id)} className="rounded-full border px-2.5 py-1 text-xs hover:bg-gray-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-600 dark:hover:bg-gray-800">{node.label}</button>
            ))}
          </div>
        </details>
      </div>
    </section>
  );
}

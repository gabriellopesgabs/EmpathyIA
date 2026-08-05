'use client';

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Maximize2, Minus, Plus, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { KnowledgeGraph, KnowledgeGraphNode } from '@/lib/knowledgeGraph';

type Point = KnowledgeGraphNode & { x: number; y: number; radius: number; degree: number };
type Position = { x: number; y: number };
type DragState =
  | { kind: 'pan'; x: number; y: number; panX: number; panY: number; moved: boolean }
  | { kind: 'node'; id: string; x: number; y: number; nodeX: number; nodeY: number; moved: boolean };

const COLORS: Record<string, string> = {
  meeting: '#3b82f6', transcript: '#64748b', summary: '#8b7aa8', note: '#737373',
  project: '#b7791f', person: '#4f8a72', tag: '#9a7184', task: '#b65f5f',
  decision: '#7967a6', topic: '#4d8492', segment: '#8b9098',
};

const KIND_LABELS: Record<string, string> = {
  meeting: 'Reunião', transcript: 'Transcrição', summary: 'Resumo', note: 'Nota',
  project: 'Projeto', person: 'Pessoa', tag: 'Tag', task: 'Tarefa', decision: 'Decisão',
  topic: 'Tema', segment: 'Trecho recente',
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

function prepareGraph(graph: KnowledgeGraph, width: number, height: number): { nodes: Point[]; edges: KnowledgeGraph['edges'] } {
  const degree = new Map<string, number>();
  graph.edges.forEach(edge => {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + edge.weight);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + edge.weight);
  });

  const selected = [...graph.nodes]
    .sort((left, right) => (degree.get(right.id) ?? 0) - (degree.get(left.id) ?? 0) || left.id.localeCompare(right.id))
    .slice(0, 90);
  const visible = new Set(selected.map(node => node.id));
  const edges = graph.edges.filter(edge => visible.has(edge.source) && visible.has(edge.target));
  const centerX = width / 2;
  const centerY = height / 2;
  const orbit = Math.max(58, Math.min(width, height) * 0.34);

  const nodes: Point[] = selected.map((node, index) => {
    const nodeDegree = degree.get(node.id) ?? 1;
    if (index === 0) {
      return { ...node, degree: nodeDegree, radius: 7, x: centerX, y: centerY };
    }
    const angle = index * 2.399963229728653;
    const layer = Math.sqrt(index / Math.max(1, selected.length - 1));
    return {
      ...node,
      degree: nodeDegree,
      radius: Math.min(11, 4.5 + Math.sqrt(Math.max(node.count, nodeDegree)) * .72),
      x: centerX + Math.cos(angle) * orbit * (.3 + layer * .72),
      y: centerY + Math.sin(angle) * orbit * (.3 + layer * .72),
    };
  });

  const byId = new Map(nodes.map((node, index) => [node.id, index]));
  const padding = 24;

  // Deterministic force relaxation: connected items stay near each other while
  // collision removal reserves actual room for nodes instead of point geometry.
  for (let iteration = 0; iteration < 110 && nodes.length > 2; iteration += 1) {
    const forces = nodes.map(() => ({ x: 0, y: 0 }));

    for (let left = 0; left < nodes.length; left += 1) {
      for (let right = left + 1; right < nodes.length; right += 1) {
        const dx = nodes[right].x - nodes[left].x || .01;
        const dy = nodes[right].y - nodes[left].y || .01;
        const distance = Math.max(1, Math.hypot(dx, dy));
        const minimum = nodes[left].radius + nodes[right].radius + 18;
        const collision = distance < minimum ? (minimum - distance) * .24 : 0;
        const repulsion = Math.min(2.2, 320 / (distance * distance));
        const force = collision + repulsion;
        const ux = dx / distance;
        const uy = dy / distance;
        forces[left].x -= ux * force;
        forces[left].y -= uy * force;
        forces[right].x += ux * force;
        forces[right].y += uy * force;
      }
    }

    edges.forEach(edge => {
      const sourceIndex = byId.get(edge.source);
      const targetIndex = byId.get(edge.target);
      if (sourceIndex === undefined || targetIndex === undefined) return;
      const source = nodes[sourceIndex];
      const target = nodes[targetIndex];
      const dx = target.x - source.x;
      const dy = target.y - source.y;
      const distance = Math.max(1, Math.hypot(dx, dy));
      const targetDistance = 72 + Math.min(30, Math.log2(edge.weight + 1) * 8);
      const spring = (distance - targetDistance) * .012;
      const ux = dx / distance;
      const uy = dy / distance;
      forces[sourceIndex].x += ux * spring;
      forces[sourceIndex].y += uy * spring;
      forces[targetIndex].x -= ux * spring;
      forces[targetIndex].y -= uy * spring;
    });

    nodes.forEach((node, index) => {
      if (index === 0) return;
      const cooling = 1 - iteration / 140;
      node.x = clamp(node.x + forces[index].x * cooling, padding, width - padding);
      node.y = clamp(node.y + forces[index].y * cooling, padding, height - padding);
    });
  }

  return { nodes, edges };
}

export function KnowledgeGraphView({
  graph, title, subtitle, live = false, statusLabel, onOpenNode, headerAction, className = '',
}: {
  graph: KnowledgeGraph;
  title: string;
  subtitle?: string;
  live?: boolean;
  statusLabel?: string;
  onOpenNode?: (node: KnowledgeGraphNode) => void;
  headerAction?: ReactNode;
  className?: string;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<DragState | null>(null);
  const [size, setSize] = useState({ width: 720, height: 360 });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [manualPositions, setManualPositions] = useState<Record<string, Position>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [draggingNodeId, setDraggingNodeId] = useState<string | null>(null);
  const [hiddenKinds, setHiddenKinds] = useState<Set<string>>(() => new Set());
  const [darkMode, setDarkMode] = useState(false);
  const kinds = useMemo(() => [...new Set(graph.nodes.map(node => node.kind))].sort(), [graph.nodes]);
  const visibleGraph = useMemo(() => ({
    ...graph,
    nodes: graph.nodes.filter(node => !hiddenKinds.has(node.kind)),
  }), [graph, hiddenKinds]);
  const prepared = useMemo(() => prepareGraph(visibleGraph, size.width, size.height), [size, visibleGraph]);
  const displayNodes = useMemo(() => prepared.nodes.map(node => ({
    ...node,
    ...(manualPositions[node.id] ?? {}),
  })), [manualPositions, prepared.nodes]);
  const selected = displayNodes.find(node => node.id === selectedId) ?? null;
  const hasRelationships = displayNodes.length > 1 && prepared.edges.length > 0;

  useEffect(() => {
    const root = document.documentElement;
    const update = () => setDarkMode(root.classList.contains('dark'));
    update();
    const observer = new MutationObserver(update);
    observer.observe(root, { attributes: true, attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const observer = new ResizeObserver(entries => {
      const width = Math.max(280, Math.floor(entries[0].contentRect.width));
      const maximum = live ? 350 : 420;
      const height = Math.max(260, Math.min(maximum, Math.floor(width * .52)));
      setSize(current => current.width === width && current.height === height ? current : { width, height });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [live]);

  const screenPoint = useCallback((node: Point) => ({
    x: size.width / 2 + (node.x - size.width / 2) * zoom + pan.x,
    y: size.height / 2 + (node.y - size.height / 2) * zoom + pan.y,
  }), [pan, size, zoom]);

  const nodeAt = useCallback((x: number, y: number) => {
    let nearest: Point | undefined;
    let distance = 22;
    for (const node of displayNodes) {
      const point = screenPoint(node);
      const candidate = Math.hypot(point.x - x, point.y - y);
      if (candidate < distance) {
        distance = candidate;
        nearest = node;
      }
    }
    return nearest;
  }, [displayNodes, screenPoint]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !hasRelationships) return;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.floor(size.width * ratio);
    canvas.height = Math.floor(size.height * ratio);
    canvas.style.width = `${size.width}px`;
    canvas.style.height = `${size.height}px`;
    const context = canvas.getContext('2d');
    if (!context) return;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, size.width, size.height);

    const byId = new Map(displayNodes.map(node => [node.id, node]));
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
      context.strokeStyle = darkMode ? '#56565b' : '#c7c7cc';
      context.globalAlpha = selectedId && source.id !== selectedId && target.id !== selectedId ? .14 : .55;
      context.lineWidth = Math.min(2.2, .65 + Math.sqrt(edge.weight) * .35);
      context.stroke();
    });
    context.globalAlpha = 1;

    displayNodes.forEach(node => {
      const point = screenPoint(node);
      const focused = node.id === selectedId;
      const hovered = node.id === hoveredId;
      context.beginPath();
      context.arc(point.x, point.y, Math.max(4.5, node.radius * zoom), 0, Math.PI * 2);
      context.fillStyle = COLORS[node.kind] ?? '#737373';
      context.globalAlpha = selectedId && !focused ? .48 : .92;
      context.fill();
      if (focused || hovered) {
        context.strokeStyle = darkMode ? '#f5f5f7' : '#1d1d1f';
        context.lineWidth = focused ? 2.5 : 1.5;
        context.stroke();
      }
    });
    context.globalAlpha = 1;

    const seenLabels = new Set<string>();
    const important = [...displayNodes]
      .sort((left, right) => right.degree - left.degree)
      .filter(node => {
        const key = node.label.trim().toLocaleLowerCase();
        if (seenLabels.has(key)) return node.id === selectedId || node.id === hoveredId;
        seenLabels.add(key);
        return true;
      })
      .slice(0, size.width < 520 ? 3 : 5);
    const candidates = [...important];
    [selectedId, hoveredId].forEach(id => {
      const node = displayNodes.find(candidate => candidate.id === id);
      if (node && !candidates.some(candidate => candidate.id === id)) candidates.push(node);
    });
    const occupied: Array<{ left: number; right: number; top: number; bottom: number }> = [];

    candidates.forEach(node => {
      const point = screenPoint(node);
      const label = node.label.length > 28 ? `${node.label.slice(0, 25)}…` : node.label;
      const focused = node.id === selectedId || node.id === hoveredId;
      context.font = `${focused ? 600 : 500} 12px ui-sans-serif, system-ui`;
      const width = context.measureText(label).width + 10;
      const height = 20;
      const distance = Math.max(13, node.radius * zoom + 10);
      const placements = [
        { x: point.x - width / 2, y: point.y + distance },
        { x: point.x - width / 2, y: point.y - distance - height },
        { x: point.x + distance, y: point.y - height / 2 },
        { x: point.x - distance - width, y: point.y - height / 2 },
      ];
      const placement = placements.find(candidate => {
        const box = { left: candidate.x, right: candidate.x + width, top: candidate.y, bottom: candidate.y + height };
        return box.left >= 4 && box.right <= size.width - 4 && box.top >= 4 && box.bottom <= size.height - 4
          && occupied.every(other => box.right < other.left || box.left > other.right || box.bottom < other.top || box.top > other.bottom);
      });
      if (!placement) return;
      occupied.push({ left: placement.x, right: placement.x + width, top: placement.y, bottom: placement.y + height });
      context.globalAlpha = selectedId && !focused ? .5 : 1;
      context.fillStyle = darkMode ? 'rgba(28,28,30,.92)' : 'rgba(255,255,255,.92)';
      context.beginPath();
      context.roundRect(placement.x, placement.y, width, height, 5);
      context.fill();
      context.fillStyle = darkMode ? '#f5f5f7' : '#1d1d1f';
      context.textAlign = 'center';
      context.textBaseline = 'middle';
      context.fillText(label, placement.x + width / 2, placement.y + height / 2 + .5);
    });
    context.globalAlpha = 1;
  }, [darkMode, displayNodes, hasRelationships, hoveredId, prepared.edges, screenPoint, selectedId, size, zoom]);

  const reset = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
    setManualPositions({});
    setSelectedId(null);
    setHoveredId(null);
    setHiddenKinds(new Set());
  };

  return (
    <section className={`knowledge-graph ${className}`}>
      <div className="knowledge-graph-header">
        <div className="min-w-0">
          <h2>
            {title}
            {(live || statusLabel) && <span className="knowledge-graph-status">{statusLabel ?? 'Ao vivo'}</span>}
          </h2>
          {subtitle && <p>{subtitle}</p>}
        </div>
        {(headerAction || hasRelationships) && (
          <div className="knowledge-graph-header-actions">
            {headerAction}
            {hasRelationships && (
              <div className="knowledge-graph-controls" aria-label="Controles do grafo">
                <button type="button" aria-label="Diminuir zoom" onClick={() => setZoom(value => Math.max(.55, value - .15))}><Minus /></button>
                <button type="button" aria-label="Restaurar visualização" onClick={reset}><RotateCcw /></button>
                <button type="button" aria-label="Aumentar zoom" onClick={() => setZoom(value => Math.min(2.2, value + .15))}><Plus /></button>
              </div>
            )}
          </div>
        )}
      </div>

      <div ref={containerRef} className="knowledge-graph-viewport">
        {hasRelationships ? (
          <canvas
            ref={canvasRef}
            role="img"
            aria-label={`${title}: ${visibleGraph.nodes.length} nós visíveis e ${prepared.edges.length} conexões. Arraste um ponto para reorganizar, ou use a lista abaixo com o teclado.`}
            data-dragging={Boolean(dragRef.current)}
            data-dragging-node={Boolean(draggingNodeId)}
            onWheel={event => {
              event.preventDefault();
              setZoom(value => clamp(value + (event.deltaY > 0 ? -.08 : .08), .55, 2.2));
            }}
            onPointerDown={event => {
              const bounds = event.currentTarget.getBoundingClientRect();
              const x = event.clientX - bounds.left;
              const y = event.clientY - bounds.top;
              const node = nodeAt(x, y);
              event.currentTarget.setPointerCapture(event.pointerId);
              if (node) {
                setSelectedId(node.id);
                setDraggingNodeId(node.id);
                dragRef.current = { kind: 'node', id: node.id, x: event.clientX, y: event.clientY, nodeX: node.x, nodeY: node.y, moved: false };
              } else {
                dragRef.current = { kind: 'pan', x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y, moved: false };
              }
            }}
            onPointerMove={event => {
              const drag = dragRef.current;
              if (!drag) {
                const bounds = event.currentTarget.getBoundingClientRect();
                setHoveredId(nodeAt(event.clientX - bounds.left, event.clientY - bounds.top)?.id ?? null);
                return;
              }
              const dx = event.clientX - drag.x;
              const dy = event.clientY - drag.y;
              if (Math.abs(dx) + Math.abs(dy) > 4) drag.moved = true;
              if (drag.kind === 'node') {
                setManualPositions(current => ({
                  ...current,
                  [drag.id]: {
                    x: clamp(drag.nodeX + dx / zoom, 18, size.width - 18),
                    y: clamp(drag.nodeY + dy / zoom, 18, size.height - 18),
                  },
                }));
              } else {
                setPan({ x: drag.panX + dx, y: drag.panY + dy });
              }
            }}
            onPointerUp={event => {
              const drag = dragRef.current;
              dragRef.current = null;
              setDraggingNodeId(null);
              if (drag?.kind === 'pan' && !drag.moved) setSelectedId(null);
              event.currentTarget.releasePointerCapture(event.pointerId);
            }}
            onPointerCancel={() => {
              dragRef.current = null;
              setDraggingNodeId(null);
            }}
            onPointerLeave={() => { if (!dragRef.current) setHoveredId(null); }}
          />
        ) : (
          <div className="knowledge-graph-empty">
            <span aria-hidden="true"><span /></span>
            <strong>O grafo começa com as relações</strong>
            <p>Os temas serão conectados conforme a conversa ganhar conteúdo.</p>
          </div>
        )}
      </div>

      {hasRelationships && (
        <div className="knowledge-graph-footer">
          <div className="knowledge-graph-kinds" aria-label="Filtrar tipos de conteúdo">
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
                data-hidden={hiddenKinds.has(kind)}
              >
                <span style={{ background: COLORS[kind] ?? '#737373' }} />{KIND_LABELS[kind] ?? kind}
              </button>
            ))}
          </div>
          {graph.truncated && <p className="knowledge-graph-caveat">Visão resumida para preservar desempenho.</p>}
          {selected && (
            <div className="knowledge-graph-selection">
              <div>
                <small>{KIND_LABELS[selected.kind] ?? selected.kind}</small>
                <strong>{selected.label}</strong>
                <span>{selected.degree} conexões</span>
              </div>
              {onOpenNode && (selected.meeting_id || selected.path) && (
                <Button size="sm" variant="ghost" onClick={() => onOpenNode(selected)}><Maximize2 /> Abrir</Button>
              )}
            </div>
          )}
          <details className="knowledge-graph-list">
            <summary>Explorar como lista</summary>
            <div>
              {[...displayNodes].sort((a, b) => b.degree - a.degree).map(node => (
                <button key={node.id} onClick={() => setSelectedId(node.id)} data-selected={node.id === selectedId}>{node.label}</button>
              ))}
            </div>
          </details>
        </div>
      )}
    </section>
  );
}

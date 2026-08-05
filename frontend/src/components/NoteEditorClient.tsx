'use client';

import React, { useState } from 'react';
import { FileText, Network } from 'lucide-react';
import { KnowledgeGraphView } from '@/components/KnowledgeGraph';
import { buildMarkdownKnowledgeGraph } from '@/lib/knowledgeGraph';
import { AugmentedNoteEditor } from '@/components/Notes/AugmentedNoteEditor';

interface NoteEditorClientProps {
  id: string;
}

export function NoteEditorClient({ id }: NoteEditorClientProps) {
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [activeView, setActiveView] = useState<'note' | 'graph'>('note');
  const noteGraph = React.useMemo(() => buildMarkdownKnowledgeGraph(content, title || 'Nota'), [content, title]);

  return (
    <article className="document-surface">
      <header className="document-toolbar">
        <div className="segmented-control" role="tablist" aria-label="Visualização da nota">
          <button type="button" role="tab" aria-selected={activeView === 'note'} data-active={activeView === 'note'} onClick={() => setActiveView('note')}>
            <FileText /> Nota
          </button>
          <button type="button" role="tab" aria-selected={activeView === 'graph'} data-active={activeView === 'graph'} onClick={() => setActiveView('graph')}>
            <Network /> Grafo
          </button>
        </div>
      </header>

      {activeView === 'note' ? (
        <AugmentedNoteEditor id={id} onContentChanged={(nextContent, nextTitle) => { setContent(nextContent); setTitle(nextTitle); }} />
      ) : (
        <div className="document-scroll p-4">
          <KnowledgeGraphView graph={noteGraph} title="Grafo da nota" subtitle="Temas e relações derivados localmente do documento Markdown." />
        </div>
      )}
    </article>
  );
}

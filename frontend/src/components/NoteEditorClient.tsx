'use client';

import React, { useState, useEffect } from 'react';
import { Clock, Users, Calendar, Tag, ArrowLeft, Save } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { getStoredNotes, saveNotesToStorage, FreeNote } from '@/components/NotesManager';

interface NoteEditorClientProps {
  id: string;
}

const sampleData: Record<string, FreeNote> = {
  'team-sync-dec-26': {
    id: 'team-sync-dec-26',
    title: 'Team Sync - Dec 26',
    createdAt: '2024-12-26',
    updatedAt: new Date().toISOString(),
    tags: ['Team Sync', 'Weekly', 'Product'],
    content: `# Meeting Summary\nTeam sync discussion about Q1 2024 goals and current project status.\n\n## Agenda Items\n1. Project Status Updates\n2. Q1 2024 Planning\n3. Team Concerns & Feedback\n\n## Key Decisions\n- Prioritized mobile app development for Q1\n- Scheduled weekly design reviews\n- Added two new features to the roadmap\n\n## Action Items\n- [ ] John: Create project timeline\n- [ ] Jane: Schedule design review meetings\n- [ ] Mike: Update documentation\n\n## Notes\n- Discussed current project bottlenecks\n- Reviewed customer feedback from last release\n- Planned resource allocation for upcoming sprint`
  },
  'product-review': {
    id: 'product-review',
    title: 'Product Review',
    createdAt: '2024-12-26',
    updatedAt: new Date().toISOString(),
    tags: ['Product', 'Review', 'Quarterly'],
    content: `# Product Review Meeting\n\n## Overview\nQuarterly product review session with stakeholders.\n\n## Discussion Points\n1. Q4 Performance Review\n2. Feature Prioritization\n3. Customer Feedback Analysis\n\n## Action Items\n- [ ] Update product roadmap\n- [ ] Schedule user research sessions\n- [ ] Review competitor analysis`
  },
  'project-ideas': {
    id: 'project-ideas',
    title: 'Project Ideas',
    createdAt: '2024-12-26',
    updatedAt: new Date().toISOString(),
    tags: ['Ideas', 'Planning'],
    content: `# Project Ideas\n\n## New Features\n1. AI-powered meeting summaries\n2. Calendar integration\n3. Team collaboration tools\n\n## Improvements\n- Enhanced search functionality\n- Better note organization\n- Real-time collaboration`
  },
  'action-items': {
    id: 'action-items',
    title: 'Action Items',
    createdAt: '2024-12-26',
    updatedAt: new Date().toISOString(),
    tags: ['Tasks', 'Todo', 'Planning'],
    content: `# Action Items\n\n## High Priority\n- [ ] Deploy v2.0 to production\n- [ ] Fix critical security issues\n- [ ] Complete user documentation\n\n## Medium Priority\n- [ ] Update dependencies\n- [ ] Implement error tracking\n- [ ] Add unit tests\n\n## Low Priority\n- [ ] Refactor legacy code\n- [ ] Improve code documentation\n- [ ] Setup development guidelines`
  }
};

export function NoteEditorClient({ id }: NoteEditorClientProps) {
  const router = useRouter();
  const [note, setNote] = useState<FreeNote | null>(null);
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');

  useEffect(() => {
    if (!id) return;
    const stored = getStoredNotes();
    let found = stored.find((n) => n.id === id) || sampleData[id];

    if (!found && id.startsWith('note-')) {
      found = {
        id,
        title: 'Nova Nota sem Título',
        createdAt: new Date().toLocaleDateString('pt-BR'),
        updatedAt: new Date().toISOString(),
        tags: ['Notas Livres'],
        content: '# Nova Nota\n\nComece a digitar suas ideias...',
      };
    }

    if (found) {
      setNote(found);
      setTitle(found.title);
      setContent(found.content);
    }
  }, [id]);

  const handleSave = () => {
    if (!note) return;
    const stored = getStoredNotes();
    const updatedNote: FreeNote = {
      ...note,
      title,
      content,
      updatedAt: new Date().toISOString(),
    };

    const existingIndex = stored.findIndex((n) => n.id === note.id);
    const newStored = [...stored];

    if (existingIndex >= 0) {
      newStored[existingIndex] = updatedNote;
    } else {
      newStored.unshift(updatedNote);
    }

    saveNotesToStorage(newStored);
    setNote(updatedNote);
    toast.success('Nota salva no disco local com sucesso!');
  };

  if (!note) {
    return <div className="p-8 text-gray-500">Nota não encontrada.</div>;
  }

  return (
    <div className="p-8 max-w-4xl mx-auto space-y-6">
      <div className="flex items-center justify-between">
        <button
          onClick={() => router.push('/')}
          className="flex items-center gap-2 text-sm text-gray-600 hover:text-gray-900 transition-colors"
        >
          <ArrowLeft className="w-4 h-4" />
          <span>Voltar para Início</span>
        </button>

        <button
          onClick={handleSave}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-semibold rounded-lg shadow-sm transition-all"
        >
          <Save className="w-4 h-4" />
          <span>Salvar Nota</span>
        </button>
      </div>

      <div className="space-y-4">
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Título da Nota..."
          className="w-full text-3xl font-bold bg-transparent border-b border-gray-200 dark:border-gray-800 pb-2 focus:outline-none focus:border-blue-500"
        />

        <div className="flex flex-wrap gap-4 text-xs text-gray-500">
          <div className="flex items-center gap-1">
            <Calendar className="w-3.5 h-3.5" />
            <span>Criado em: {note.createdAt}</span>
          </div>
          {note.tags && note.tags.length > 0 && (
            <div className="flex items-center gap-1">
              <Tag className="w-3.5 h-3.5" />
              <span>{note.tags.join(', ')}</span>
            </div>
          )}
        </div>
      </div>

      <div className="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl p-6 shadow-sm">
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Escreva suas notas em Markdown aqui..."
          rows={18}
          className="w-full font-mono text-sm bg-transparent resize-y focus:outline-none text-gray-800 dark:text-gray-200"
        />
      </div>
    </div>
  );
}

'use client';

import React, { useState, useEffect } from 'react';
import { Archive, ArchiveRestore, ChevronDown, ChevronRight, Plus, Trash2, Pencil, NotebookPen } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { ConfirmationModal } from './ConfirmationModel/confirmation-modal';

export interface FreeNote {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  tags: string[];
  content: string;
  archivedAt?: string | null;
}

interface NotesManagerProps {
  searchQuery?: string;
  recordedNotes?: React.ReactNode;
  archivedRecordedNotes?: React.ReactNode;
  activeRecordedCount?: number;
  archivedRecordedCount?: number;
}

const STORAGE_KEY = 'empathy_free_notes';

export function getStoredNotes(): FreeNote[] {
  if (typeof window === 'undefined') return [];
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved ? JSON.parse(saved) : [];
  } catch (err) {
    console.error('Failed to parse notes from storage:', err);
    return [];
  }
}

export function saveNotesToStorage(notes: FreeNote[]) {
  if (typeof window === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(notes));
  } catch (err) {
    console.error('Failed to save notes:', err);
  }
}

export function NotesManager({
  searchQuery = '',
  recordedNotes,
  archivedRecordedNotes,
  activeRecordedCount = 0,
  archivedRecordedCount = 0,
}: NotesManagerProps) {
  const router = useRouter();
  const [notes, setNotes] = useState<FreeNote[]>([]);
  const [showArchived, setShowArchived] = useState(false);
  const [pendingDeletion, setPendingDeletion] = useState<FreeNote | null>(null);
  const normalizedQuery = searchQuery.trim().toLocaleLowerCase('pt-BR');
  const visibleNotes = normalizedQuery
    ? notes.filter(note => `${note.title}\n${note.content}`.toLocaleLowerCase('pt-BR').includes(normalizedQuery))
    : notes;
  const activeNotes = visibleNotes.filter(note => !note.archivedAt);
  const archivedNotes = visibleNotes.filter(note => Boolean(note.archivedAt));
  const totalArchived = archivedNotes.length + archivedRecordedCount;

  useEffect(() => {
    setNotes(getStoredNotes());
  }, []);

  const persistNotes = (updated: FreeNote[]) => {
    setNotes(updated);
    saveNotesToStorage(updated);
  };

  const handleCreateNewNote = () => {
    const newId = `note-${Date.now()}`;
    const newNote: FreeNote = {
      id: newId,
      title: 'Nova Nota sem Título',
      createdAt: new Date().toLocaleDateString('pt-BR'),
      updatedAt: new Date().toISOString(),
      tags: ['Nota'],
      content: '# Nova Nota\n\nComece a digitar suas ideias ou conecte uma gravação...',
      archivedAt: null,
    };

    persistNotes([newNote, ...notes]);
    toast.success('Nota criada com sucesso!');
    router.push(`/notes?id=${newId}`);
  };

  const setNoteArchived = (id: string, archived: boolean) => {
    persistNotes(notes.map(note => note.id === id ? {
      ...note,
      archivedAt: archived ? new Date().toISOString() : null,
      updatedAt: new Date().toISOString(),
    } : note));
    toast.success(archived ? 'Nota arquivada' : 'Nota restaurada');
  };

  const permanentlyDeleteNote = (id: string) => {
    persistNotes(notes.filter(note => note.id !== id));
    setPendingDeletion(null);
    toast.info('Nota excluída');
  };

  const renderNote = (note: FreeNote, archived: boolean) => (
    <div
      key={note.id}
      onClick={() => router.push(`/notes?id=${note.id}`)}
      className="group flex items-center justify-between px-3 py-2 text-xs rounded-lg text-gray-700 dark:text-gray-200 hover:bg-black/5 dark:hover:bg-white/5 cursor-pointer transition-all"
    >
      <div className="flex min-w-0 items-center gap-2">
        <Pencil className="w-3.5 h-3.5 text-gray-500 group-hover:text-blue-500 transition-colors flex-shrink-0" aria-label="Nota escrita ou editada" />
        <span className="truncate font-medium">{note.title}</span>
      </div>
      <div className="ml-2 flex items-center opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
        <button
          onClick={(event) => {
            event.stopPropagation();
            setNoteArchived(note.id, !archived);
          }}
          className="p-1 text-gray-400 hover:text-blue-600 rounded"
          aria-label={archived ? 'Restaurar nota' : 'Arquivar nota'}
        >
          {archived ? <ArchiveRestore className="w-3.5 h-3.5" /> : <Archive className="w-3.5 h-3.5" />}
        </button>
        <button
          onClick={(event) => {
            event.stopPropagation();
            setPendingDeletion(note);
          }}
          className="p-1 text-gray-400 hover:text-red-500 rounded"
          aria-label="Excluir nota"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between px-2 py-1.5 text-xs font-semibold text-gray-500 uppercase tracking-wider">
        <span className="flex items-center gap-1.5">
          <NotebookPen className="w-3.5 h-3.5 text-gray-500" />
          <span>Notas</span>
        </span>
        <button
          onClick={handleCreateNewNote}
          className="p-1 hover:bg-black/5 dark:hover:bg-white/10 rounded-md transition-colors text-gray-600 dark:text-gray-300"
          title="Criar nova nota"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      <div className="space-y-1">
        {activeNotes.length === 0 && activeRecordedCount === 0 ? (
          <div className="px-3 py-2 text-xs text-gray-400 italic">
            {normalizedQuery ? 'Nenhuma nota encontrada.' : 'Nenhuma nota criada.'}
          </div>
        ) : (
          <>
            {activeNotes.map(note => renderNote(note, false))}
            {recordedNotes}
          </>
        )}
      </div>

      {totalArchived > 0 && (
        <div className="pt-2">
          <button
            onClick={() => setShowArchived(current => !current)}
            className="flex w-full items-center gap-1.5 px-2 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            aria-expanded={showArchived}
          >
            {showArchived ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
            <span>Arquivadas</span>
            <span className="ml-auto rounded-full bg-gray-200 px-1.5 py-0.5 text-[10px] dark:bg-gray-800">{totalArchived}</span>
          </button>
          {showArchived && (
            <div className="mt-1 space-y-1">
              {archivedNotes.map(note => renderNote(note, true))}
              {archivedRecordedNotes}
            </div>
          )}
        </div>
      )}

      <ConfirmationModal
        isOpen={Boolean(pendingDeletion)}
        isArchived={Boolean(pendingDeletion?.archivedAt)}
        text={pendingDeletion?.archivedAt
          ? 'A nota já está arquivada. Excluí-la removerá definitivamente o conteúdo escrito deste dispositivo.'
          : 'Você pode arquivar para retirar a nota da lista principal sem perder o conteúdo, ou excluí-la definitivamente.'}
        onArchive={pendingDeletion ? () => {
          setNoteArchived(pendingDeletion.id, true);
          setPendingDeletion(null);
        } : undefined}
        onConfirm={() => pendingDeletion && permanentlyDeleteNote(pendingDeletion.id)}
        onCancel={() => setPendingDeletion(null)}
      />
    </div>
  );
}

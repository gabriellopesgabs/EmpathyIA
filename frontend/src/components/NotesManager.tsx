'use client';

import React, { useState, useEffect } from 'react';
import { Plus, StickyNote, Trash2, Calendar, FileText } from 'lucide-react';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';

export interface FreeNote {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  tags: string[];
  content: string;
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

export function NotesManager() {
  const router = useRouter();
  const [notes, setNotes] = useState<FreeNote[]>([]);

  useEffect(() => {
    setNotes(getStoredNotes());
  }, []);

  const handleCreateNewNote = () => {
    const newId = `note-${Date.now()}`;
    const newNote: FreeNote = {
      id: newId,
      title: 'Nova Nota sem Título',
      createdAt: new Date().toLocaleDateString('pt-BR'),
      updatedAt: new Date().toISOString(),
      tags: ['Notas Livres'],
      content: '# Nova Nota\n\nComece a digitar suas ideias ou conecte uma gravação...',
    };

    const updated = [newNote, ...notes];
    setNotes(updated);
    saveNotesToStorage(updated);
    toast.success('Nota criada com sucesso!');
    router.push(`/notes?id=${newId}`);
  };

  const handleDeleteNote = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    const updated = notes.filter((n) => n.id !== id);
    setNotes(updated);
    saveNotesToStorage(updated);
    toast.info('Nota removida');
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between px-2 py-1.5 text-xs font-semibold text-gray-500 uppercase tracking-wider">
        <span className="flex items-center gap-1.5">
          <StickyNote className="w-3.5 h-3.5 text-amber-500" />
          <span>Minhas Notas Livres</span>
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
        {notes.length === 0 ? (
          <div className="px-3 py-2 text-xs text-gray-400 italic">
            Nenhuma nota avulsa criada.
          </div>
        ) : (
          notes.map((note) => (
            <div
              key={note.id}
              onClick={() => router.push(`/notes?id=${note.id}`)}
              className="group flex items-center justify-between px-3 py-2 text-xs rounded-lg text-gray-700 dark:text-gray-200 hover:bg-black/5 dark:hover:bg-white/5 cursor-pointer transition-all"
            >
              <div className="flex items-center gap-2 truncate">
                <FileText className="w-3.5 h-3.5 text-gray-400 group-hover:text-blue-500 transition-colors flex-shrink-0" />
                <span className="truncate font-medium">{note.title}</span>
              </div>

              <button
                onClick={(e) => handleDeleteNote(e, note.id)}
                className="opacity-0 group-hover:opacity-100 p-1 text-gray-400 hover:text-red-500 transition-all"
                title="Excluir nota"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

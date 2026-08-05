import { invoke } from '@tauri-apps/api/core';
import type { OutlookNoteMeetingContext } from '@/types/integrations';

export interface NoteDocument {
  id: string;
  title: string;
  content: string;
  created_at: string;
  updated_at: string;
  recorded: boolean;
  written: boolean;
  archived: boolean;
  folder_path: string;
  external_meeting?: OutlookNoteMeetingContext | null;
  content_hash: string;
}

interface LegacyNote {
  id: string;
  title: string;
  content: string;
  createdAt?: string;
  updatedAt?: string;
}

const LEGACY_STORAGE_KEY = 'empathy_free_notes';
const MIGRATION_KEY = 'empathy_markdown_notes_migrated_v1';
let legacyMigrationPromise: Promise<number> | null = null;

export const noteService = {
  async create(title = 'Nova nota', content = '# Nova nota\n\nComece a escrever…', createdAt?: string) {
    return invoke<{ id: string; folder_path: string }>('api_create_note', {
      title,
      content,
      createdAt: createdAt ?? null,
    });
  },

  async get(id: string) {
    return invoke<NoteDocument>('api_get_note', { meetingId: id });
  },

  async save(id: string, title: string, content: string, expectedHash?: string) {
    return invoke<NoteDocument>('api_save_note', { meetingId: id, title, content, expectedHash: expectedHash ?? null });
  },

  async setArchived(id: string, archived: boolean) {
    return invoke('api_set_meeting_archived', { meetingId: id, archived });
  },

  async remove(id: string) {
    return invoke('api_delete_meeting', { meetingId: id, authToken: null });
  },

  async migrateLegacyNotes(): Promise<number> {
    if (legacyMigrationPromise) return legacyMigrationPromise;
    legacyMigrationPromise = (async () => {
      if (typeof window === 'undefined' || localStorage.getItem(MIGRATION_KEY) === 'done') return 0;
      const raw = localStorage.getItem(LEGACY_STORAGE_KEY);
      if (!raw) {
        localStorage.setItem(MIGRATION_KEY, 'done');
        return 0;
      }

      let notes: LegacyNote[];
      try {
        notes = JSON.parse(raw) as LegacyNote[];
      } catch {
        return 0;
      }

      let migrated = 0;
      const pending = [...notes];
      while (pending.length > 0) {
        const note = pending[0];
        await noteService.create(note.title || 'Nota sem título', note.content || '', note.updatedAt || note.createdAt);
        migrated += 1;
        pending.shift();
        localStorage.setItem(LEGACY_STORAGE_KEY, JSON.stringify(pending));
      }
      localStorage.removeItem(LEGACY_STORAGE_KEY);
      localStorage.setItem(MIGRATION_KEY, 'done');
      return migrated;
    })();
    try {
      return await legacyMigrationPromise;
    } catch (error) {
      legacyMigrationPromise = null;
      throw error;
    }
  },
};

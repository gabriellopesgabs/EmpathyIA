'use client';

import { Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import { NoteEditorClient } from '@/components/NoteEditorClient';
import { LoaderIcon } from 'lucide-react';

function NoteContent() {
  const searchParams = useSearchParams();
  const id = searchParams.get('id') || '';
  return <NoteEditorClient id={id} />;
}

export default function NotesPage() {
  return (
    <Suspense
      fallback={
        <div className="flex items-center justify-center h-screen bg-gray-50 dark:bg-gray-900">
          <LoaderIcon className="w-8 h-8 animate-spin text-blue-600" />
        </div>
      }
    >
      <NoteContent />
    </Suspense>
  );
}

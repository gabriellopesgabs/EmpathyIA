import { NoteEditorClient } from '@/components/NoteEditorClient';

export const dynamicParams = true;

export function generateStaticParams() {
  return [
    { id: 'team-sync-dec-26' },
    { id: 'product-review' },
    { id: 'project-ideas' },
    { id: 'action-items' }
  ];
}

interface PageProps {
  params: {
    id: string;
  };
}

export default function NotePage({ params }: PageProps) {
  return <NoteEditorClient id={params.id} />;
}

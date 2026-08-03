import { NoteEditorClient } from '@/components/NoteEditorClient';

export const dynamicParams = false;

export function generateStaticParams() {
  return [
    { id: 'team-sync-dec-26' },
    { id: 'product-review' },
    { id: 'project-ideas' },
    { id: 'action-items' }
  ];
}

interface PageProps {
  params: Promise<{
    id: string;
  }>;
}

export default async function NotePage({ params }: PageProps) {
  const { id } = await params;
  return <NoteEditorClient id={id} />;
}

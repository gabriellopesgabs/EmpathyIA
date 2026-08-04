'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { Link2 } from 'lucide-react';

type RelatedMeeting = {
  meeting_id: string;
  title: string;
  path: string;
  reasons: string[];
  score: number;
};

export function RelatedMeetings({ meetingId }: { meetingId: string }) {
  const router = useRouter();
  const [meetings, setMeetings] = useState<RelatedMeeting[]>([]);

  useEffect(() => {
    let cancelled = false;
    const load = () => invoke<RelatedMeeting[]>('api_get_related_meetings', { meetingId })
      .then(items => { if (!cancelled) setMeetings(items); })
      .catch(error => console.warn('Could not load related meetings:', error));
    load();
    window.addEventListener('knowledge-index-updated', load);
    return () => {
      cancelled = true;
      window.removeEventListener('knowledge-index-updated', load);
    };
  }, [meetingId]);

  if (meetings.length === 0) return null;

  return (
    <section className="mx-6 mb-6 rounded-xl border bg-gray-50 p-4">
      <h3 className="mb-3 flex items-center gap-2 text-sm font-semibold text-gray-700">
        <Link2 className="h-4 w-4" /> Reuniões relacionadas
      </h3>
      <div className="grid gap-2 sm:grid-cols-2">
        {meetings.map(meeting => (
          <button key={meeting.meeting_id} onClick={() => router.push(`/meeting-details?id=${meeting.meeting_id}`)}
            className="rounded-lg border bg-white p-3 text-left transition-colors hover:bg-blue-50">
            <p className="text-sm font-medium text-gray-900">{meeting.title}</p>
            <p className="mt-1 line-clamp-2 text-xs text-gray-500">{meeting.reasons.join(' · ')}</p>
          </button>
        ))}
      </div>
    </section>
  );
}

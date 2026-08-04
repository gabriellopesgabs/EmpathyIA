import { describe, expect, it } from 'vitest';
import { mergeLiveTranscript } from '@/lib/liveTranscript';
import type { TranscriptUpdate } from '@/types';

function update(sequenceId: number, start: number, text: string, partial = true): TranscriptUpdate {
  return {
    sequence_id: sequenceId,
    chunk_start_time: start,
    audio_start_time: start,
    audio_end_time: start + 2,
    duration: 2,
    text,
    timestamp: '10:00',
    source: 'Audio',
    is_partial: partial,
    confidence: 0.9,
  };
}

describe('live transcript merging', () => {
  it('renders out-of-order events in recording order', () => {
    const later = mergeLiveTranscript([], update(2, 4, 'segundo'));
    const ordered = mergeLiveTranscript(later, update(1, 2, 'primeiro'));
    expect(ordered.map(segment => segment.text)).toEqual(['primeiro', 'segundo']);
  });

  it('replaces the same sequence when a refined result arrives', () => {
    const partial = mergeLiveTranscript([], update(1, 2, 'texto parcial'));
    const refined = mergeLiveTranscript(partial, update(1, 2, 'texto final', false));
    expect(refined).toHaveLength(1);
    expect(refined[0]).toMatchObject({ text: 'texto final', is_partial: false });
  });
});

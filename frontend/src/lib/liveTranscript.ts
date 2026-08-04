import type { Transcript, TranscriptUpdate } from '@/types';

export function mergeLiveTranscript(
  previous: Transcript[],
  update: TranscriptUpdate,
): Transcript[] {
  const incoming: Transcript = {
    id: `segment-${update.sequence_id}`,
    text: update.text,
    timestamp: update.timestamp,
    sequence_id: update.sequence_id,
    chunk_start_time: update.chunk_start_time,
    is_partial: update.is_partial,
    confidence: update.confidence,
    audio_start_time: update.audio_start_time,
    audio_end_time: update.audio_end_time,
    duration: update.duration,
  };
  const existingIndex = previous.findIndex(item => item.sequence_id === update.sequence_id);
  const next = existingIndex >= 0
    ? previous.map((item, index) => index === existingIndex ? incoming : item)
    : [...previous, incoming];

  return next.sort((left, right) => {
    const timeDifference = (left.audio_start_time ?? left.chunk_start_time ?? 0)
      - (right.audio_start_time ?? right.chunk_start_time ?? 0);
    return timeDifference || (left.sequence_id ?? 0) - (right.sequence_id ?? 0);
  });
}

export interface ParticipantSourceReceipt {
  provider: string;
  source_kind: string;
  source_id: string;
  note_id: string;
  observed_at: string;
}

export interface ParticipantMemory {
  schema: 1;
  id: string;
  display_name: string;
  emails: string[];
  aliases: string[];
  organization?: string | null;
  role?: string | null;
  confirmed_fields: string[];
  source_receipts: ParticipantSourceReceipt[];
  created_at: string;
  updated_at: string;
  merged_into?: string | null;
  notes: string;
  hypotheses: string;
  path: string;
}

export interface ParticipantMemoryUpdate {
  id: string;
  display_name: string;
  emails: string[];
  aliases: string[];
  organization?: string | null;
  role?: string | null;
  notes: string;
  hypotheses: string;
  expected_updated_at: string;
}

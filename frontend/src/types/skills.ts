export type SkillLayer = 'individual' | 'collective' | 'artificial';

export interface SkillContextPermissions {
  selection: boolean;
  note: boolean;
  transcript: boolean;
  related_notes: boolean;
}

export interface SkillDefinition {
  schema: 1;
  id: string;
  name: string;
  description: string;
  layer: SkillLayer;
  instruction: string;
  default_title: string;
  context: SkillContextPermissions;
}

export interface SkillInfo extends SkillDefinition { native: boolean }
export interface RelatedSkillDocument { id: string; title: string; content: string }
export interface SkillContextRequest {
  note_id: string; note_title: string; note: string; selection?: string | null;
  transcript?: string | null; related_notes: RelatedSkillDocument[];
}
export interface SkillRunResult {
  run_id: string; skill_id: string; skill_name: string; layer: SkillLayer;
  title: string; markdown: string; provider: string; model: string;
  source_scope: 'selection' | 'note' | 'transcript'; external: boolean; context_documents: string[];
}
export interface SkillResultMetadata {
  id: string; skill_id: string; skill_name: string; layer: SkillLayer;
  created_at: string; source_scope: 'selection' | 'note' | 'transcript'; provider: string;
  model: string; context_documents: string[];
}

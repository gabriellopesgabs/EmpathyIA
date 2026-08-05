import { describe, expect, it } from 'vitest';
import { buildSkillResultBlock, insertSkillResult, parseSkillBlocks } from '../../src/lib/skillBlocks';

const result = { run_id: 'run', skill_id: 'clarify-thinking', skill_name: 'Clarificar pensamento', layer: 'individual' as const, title: 'Pensamento clarificado', markdown: 'Texto', provider: 'builtin-ai', model: 'qwen', source_scope: 'selection' as const, external: false, context_documents: ['note-1'] };

describe('skill result blocks', () => {
  it('creates signed, parseable Markdown', () => {
    const block = buildSkillResultBlock(result, result.title, result.markdown, 'fixed-id', '2026-08-05T00:00:00Z');
    expect(block).toContain('*Skill (Clarificar pensamento)*');
    expect(parseSkillBlocks(block)[0]).toMatchObject({ id: 'fixed-id', layer: 'individual' });
  });
  it('inserts after an unchanged selection', () => {
    const inserted = insertSkillResult('antes alvo depois', 'BLOCO', { start: 6, end: 10, text: 'alvo' });
    expect(inserted.afterSelection).toBe(true); expect(inserted.content).toContain('alvo\n\nBLOCO');
  });
  it('falls back to the end when selection changed', () => {
    const inserted = insertSkillResult('texto alterado', 'BLOCO', { start: 0, end: 5, text: 'outro' });
    expect(inserted.afterSelection).toBe(false); expect(inserted.content.endsWith('BLOCO')).toBe(true);
  });
});

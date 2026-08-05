import type { SkillRunResult } from '@/types/skills';

export type TextSelection = { start: number; end: number; text: string };

export function buildSkillResultBlock(result: SkillRunResult, title: string, markdown: string, id = crypto.randomUUID(), createdAt = new Date().toISOString()) {
  return `<!-- empathy-skill-result
id: ${id}
skill_id: ${result.skill_id}
skill_name: ${result.skill_name}
layer: ${result.layer}
created_at: ${createdAt}
source_scope: ${result.source_scope}
provider: ${result.provider}
model: ${result.model}
context_documents: ${JSON.stringify(result.context_documents)}
-->
## ${title.trim() || result.title}

*Skill (${result.skill_name})*

${markdown.trim()}
<!-- /empathy-skill-result -->`;
}

export function insertSkillResult(content: string, block: string, selection?: TextSelection | null) {
  if (selection && selection.text && content.slice(selection.start, selection.end) === selection.text) {
    const before = content.slice(0, selection.end).trimEnd();
    const after = content.slice(selection.end).trimStart();
    return { content: `${before}\n\n${block}${after ? `\n\n${after}` : ''}`, afterSelection: true };
  }
  return { content: `${content.trimEnd()}${content.trim() ? '\n\n' : ''}${block}`, afterSelection: false };
}

export function parseSkillBlocks(markdown: string) {
  const expression = /<!-- empathy-skill-result\n([\s\S]*?)-->\n([\s\S]*?)<!-- \/empathy-skill-result -->/g;
  return [...markdown.matchAll(expression)].map(match => {
    const metadata = Object.fromEntries(match[1].split('\n').map(line => {
      const index = line.indexOf(':'); return index < 0 ? [line, ''] : [line.slice(0, index).trim(), line.slice(index + 1).trim()];
    }));
    const title = match[2].match(/^##\s+(.+)$/m)?.[1] ?? metadata.skill_name ?? 'Resultado de Skill';
    return { id: metadata.id, skillId: metadata.skill_id, skillName: metadata.skill_name, layer: metadata.layer, title, contextDocuments: (() => { try { return JSON.parse(metadata.context_documents ?? '[]') as string[]; } catch { return []; } })() };
  });
}

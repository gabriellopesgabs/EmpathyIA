# Workspace de conhecimento

O EmpathyIA trata os arquivos Markdown como fonte de verdade. O SQLite mantém somente um índice
reconstruível para busca, relações, tarefas, decisões e painéis.

## Schema de reunião

Novas reuniões usam `empathy_schema: 2` em `meeting.md`:

```yaml
---
empathy_schema: 2
type: meeting
id: "uuid-estável"
title: "Reunião de produto"
created_at: "2026-08-03T10:00:00Z"
updated_at: "2026-08-03T11:00:00Z"
project: "EmpathyIA"
participants: ["Gabriel", "Maria"]
tags: [meeting, produto]
status: completed
---
```

Campos desconhecidos são preservados quando o EmpathyIA atualiza título ou propriedades. Links
Markdown padrão são preferidos; Wikilinks podem ser lidos, mas não são necessários.

## Índice reconstruível

**Conhecimento → Reindexar** percorre o workspace e recria:

- documentos e propriedades;
- links e backlinks;
- tarefas Markdown (`- [ ]` e `- [x]`);
- itens sob seções de decisões;
- relações por projeto, participantes e tags.

Um monitor de arquivos solicita nova indexação depois de edições externas. Excluir o SQLite não
exclui os documentos, mas configurações e estados operacionais ainda usam esse banco.

## Busca

Filtros suportados:

```text
project:"Empathy IA" person:Gabriel has:decision atualização
tag:produto
kind:summary has:task
```

Também é possível abrir a paleta global com `Cmd/Ctrl + K`.

## Importação e Canvas

O importador copia pastas de Markdown, texto, HTML, VTT/SRT, CSV e JSON para um lote único em `Importados/`. A estrutura
relativa e anexos comuns são preservados; os originais nunca são alterados. A exportação JSON Canvas
cria arquivos `.canvas` em `Canvases/` usando caminhos relativos.

## Extensões seguras

Manifestos ficam em `.empathy/extensions/<id>/manifest.json`. O EmpathyIA não carrega JavaScript nem
binários desses diretórios. Somente estas ações declarativas são aceitas:

- `reindex`;
- `export_canvas`, com `config.project` opcional;
- `create_digest`.

Exemplo:

```json
{
  "id": "digest-diario",
  "name": "Digest diário",
  "description": "Cria um Markdown com ações e decisões recentes.",
  "action": "create_digest",
  "config": {},
  "enabled": false
}
```

Uma extensão precisa ser validada e ativada explicitamente antes de ser executada.

## Captura da web

A tela **Capturar contexto** grava uma URL e conteúdo em `Contextos/`. A extensão opcional em
`browser-extension/` captura a seleção do navegador e baixa um Markdown sem usar servidor; esse
arquivo pode ser importado pelo EmpathyIA.

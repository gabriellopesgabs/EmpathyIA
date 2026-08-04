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
note_origins: [recorded]
---
```

Campos desconhecidos são preservados quando o EmpathyIA atualiza título ou propriedades. Links
Markdown padrão são preferidos; Wikilinks podem ser lidos, mas não são necessários.

## Origem das notas

O aplicativo apresenta notas escritas e reuniões gravadas em uma única coleção **Notas**. Os
ícones indicam a origem do conteúdo:

- círculo com ponto: conteúdo originado de uma gravação ou áudio importado;
- lápis: nota escrita ou editada manualmente;
- os dois ícones: uma gravação que também recebeu contribuição humana.

Reuniões novas começam com `note_origins: [recorded]`. Ao editar título, resumo ou propriedades,
o EmpathyIA acrescenta `written` ao campo, preservando o corpo e os demais campos do `meeting.md`.
Arquivos antigos sem `note_origins` continuam compatíveis e são tratados como gravados.

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

## Grafos de conhecimento

O Empathy oferece três visualizações conectadas:

- **Transcrição → Dividido** mostra a transcrição em tempo real ao lado dos temas que surgem na
  conversa. Trechos parciais não entram no grafo e nada adicional é persistido enquanto a pessoa
  ainda está falando;
- **Grafo desta reunião**, nos detalhes de uma reunião salva, conecta o arquivo da reunião à
  transcrição, resumo, projeto, participantes, tags, tarefas e decisões;
- **Conhecimento → Grafo global** reúne todas as reuniões e seus vínculos derivados do índice.

O grafo usa Canvas 2D, limita automaticamente uma visão muito grande e mantém uma lista navegável
por teclado como alternativa acessível. Arrastar move a visualização; os botões controlam zoom e
restauração sem capturar a rolagem do sistema. As posições são determinísticas para reduzir mudanças
visuais quando novos trechos ou documentos aparecem.

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

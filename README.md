# EmpathyIA

Assistente desktop de reuniões com transcrição local, organização por pastas e documentos Markdown portáveis.

**Inteligência individual + coletiva + artificial para ampliar o pensamento humano.** O Empathy
preserva a contribuição humana como fonte principal, conecta a memória local e aplica Skills que
sempre precisam ser revisadas antes de entrar na nota.

> O EmpathyIA está em desenvolvimento. Use apenas os artefatos publicados em
> [gabriellopesgabs/EmpathyIA Releases](https://github.com/gabriellopesgabs/EmpathyIA/releases).
> O aplicativo consulta apenas o canal próprio do EmpathyIA e só instala pacotes cuja assinatura
> corresponda à chave pública incorporada ao aplicativo.

## Privacidade em linguagem direta

- Gravações e transcrições são processadas localmente e salvas nas pastas escolhidas pelo usuário.
- Cada reunião possui `meeting.md` e `transcript.md`, além do áudio e metadados técnicos. O
  `summary.md` de versões anteriores é preservado e incorporado à nota somente na primeira edição.
- O conteúdo portátil fica nos arquivos Markdown. O SQLite continua sendo usado como catálogo
  operacional para busca, paginação e processamento e ainda é necessário para a interface atual.
- Chaves de API ficam no cofre seguro do sistema operacional, não no SQLite.
- Modelos locais não enviam o conteúdo da reunião para terceiros.
- Ao escolher OpenAI, Anthropic, Groq, OpenRouter ou outro endpoint externo, o texto necessário
  para gerar o resumo é enviado ao provedor selecionado. Essa escolha deve ser consciente.

Leia a [Política de Privacidade](PRIVACY_POLICY.md) antes de usar provedores externos.

## Estrutura de uma reunião

```text
Minhas reuniões/
└── Nome da reunião/
    ├── meeting.md
    ├── transcript.md
    ├── summary.md
    ├── metadata.json
    └── audio.mp4
```

Os arquivos Markdown são feitos para continuar úteis fora do EmpathyIA e podem ser versionados,
copiados ou abertos em editores como Obsidian.

Notas escritas e reuniões gravadas aparecem na mesma biblioteca **Notas**. O ícone de gravação
identifica conteúdo capturado; o lápis identifica conteúdo escrito ou editado. Uma nota pode exibir
os dois. Arquivar é reversível e excluir exige confirmação, oferecendo o arquivamento como opção.

## Interface e atalhos

A interface desktop acompanha automaticamente o tema claro ou escuro do sistema e organiza o app
em navegação, biblioteca e documento. Durante uma gravação, é possível alternar entre transcrição,
grafo ao vivo ou as duas visualizações lado a lado.

- `Cmd/Ctrl + N`: nova nota Markdown
- `Cmd/Ctrl + Shift + R`: iniciar ou encerrar gravação
- `Cmd/Ctrl + K`: busca e comandos
- `Cmd/Ctrl + Shift + K`: abrir Conhecimento
- `Cmd/Ctrl + ,`: abrir Configurações
- `Cmd/Ctrl + S`: salvar a nota atual

## Skills e autoria humana

Em qualquer nota, selecione um trecho ou use a nota inteira e abra **Skills** com `Cmd/Ctrl + K`
ou digitando `/skill`. O painel lateral mostra a camada da Skill, o provedor e todo contexto que
será processado. Transcrição e até cinco notas relacionadas são sempre opcionais e explícitas.

O resultado é editável antes da inserção. Ao confirmar, o Empathy acrescenta um novo bloco Markdown
assinado com nome da Skill, data, modelo e documentos usados; resultados anteriores nunca são
substituídos silenciosamente. Skills nativas podem ser duplicadas e as personalizadas ficam em
`Application Support/Empathy/skills` como JSON portável, sem uma nova tabela SQL.

### Preparação de reuniões

Uma reunião do Outlook pode virar uma Nota antes de começar. A Skill nativa
**Preparar reunião** combina a Nota, os participantes confirmados e, somente se
o usuário escolher, até dez mensagens relacionadas. A prévia informa se o
modelo é local ou externo e quais fontes serão processadas. O resultado só
entra no Markdown depois da revisão humana.

A memória de participantes fica em `People/*.md`, separando fatos confirmados
de hipóteses. Ela é portátil, corrigível, mesclável e removível, e também
enriquece o grafo sem criar uma base de perfilamento paralela.

### Presença de IA nas chamadas

O Teams usa um participante nomeado **Empathy AI — gravação e transcrição** e
uma trilha `agent-audit.md`. Convite, lobby, consentimento, transcrição, pausa e
saída são estados explícitos. O runtime de mídia é um serviço Windows/Azure
separado e a ação permanece bloqueada enquanto ele não comprovar readiness.

Zoom e Google Meet não são apresentados como equivalentes. Zoom RTMS usa o
indicador nativo da plataforma; Meet REST importa participantes/artefatos sem
entrar na chamada; Meet Media API é experimental. Configurações mostra as
capacidades reais e cada requisito ainda pendente.

## Memória conectada

A área **Conhecimento** transforma os documentos locais em uma memória navegável de projetos,
pessoas, decisões, tarefas e reuniões relacionadas. Ela inclui busca global (`Cmd/Ctrl + K`),
monitoramento de edições externas, importação de pastas, captura de contexto da web, exportação
JSON Canvas e automações declarativas que não executam código de terceiros.

O índice pode ser reconstruído a partir dos arquivos a qualquer momento. Consulte a
[documentação do workspace de conhecimento](docs/KNOWLEDGE_WORKSPACE.md).

## Desenvolvimento

Pré-requisitos:

- Node.js 20 ou superior
- pnpm 11.9.0
- Rust 1.88 ou superior
- CMake
- FFmpeg instalado localmente ou informado por `EMPATHY_FFMPEG_PATH`

```bash
git clone https://github.com/gabriellopesgabs/EmpathyIA.git
cd EmpathyIA/frontend
pnpm install --frozen-lockfile
pnpm run check
pnpm run tauri:dev
```

O build não baixa executáveis durante a compilação. Builds de distribuição devem provisionar um
FFmpeg revisado por `EMPATHY_FFMPEG_PATH`.

## Segurança de distribuição

- Não existe fallback para o atualizador ou para chaves de assinatura do projeto upstream.
- Releases oficiais pertencem exclusivamente a `gabriellopesgabs/EmpathyIA`.
- O manifesto `latest.json` e seus pacotes são gerados pelo workflow de release com a chave própria
  do EmpathyIA. A publicação continua sendo uma ação manual depois da revisão do rascunho.
- Builds de teste são artefatos de CI, não releases públicas. Consulte
  [o procedimento de release](docs/RELEASING.md) antes de distribuir uma versão.

## Origem e licença

EmpathyIA é derivado do projeto open source Meetily. A atribuição original permanece preservada em
[LICENSE.md](LICENSE.md). Modificações do EmpathyIA são mantidas por Gabriel Lopes e colaboradores.

Este projeto é distribuído sob a licença MIT. A atribuição histórica não implica endosso,
distribuição ou suporte pelo projeto upstream.

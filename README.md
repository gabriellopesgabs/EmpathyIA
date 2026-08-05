# EmpathyIA

Assistente desktop de reuniões com transcrição local, organização por pastas e documentos Markdown portáveis.

> O EmpathyIA está em desenvolvimento. Use apenas os artefatos publicados em
> [gabriellopesgabs/EmpathyIA Releases](https://github.com/gabriellopesgabs/EmpathyIA/releases).
> O aplicativo consulta apenas o canal próprio do EmpathyIA e só instala pacotes cuja assinatura
> corresponda à chave pública incorporada ao aplicativo.

## Privacidade em linguagem direta

- Gravações e transcrições são processadas localmente e salvas nas pastas escolhidas pelo usuário.
- Cada reunião possui `meeting.md`, `transcript.md` e `summary.md`, além do áudio e metadados técnicos.
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

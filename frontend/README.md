# EmpathyIA Desktop

Aplicativo desktop em Next.js e Tauri para gravação, transcrição local e organização de reuniões.

## Requisitos

- Node.js 20 ou superior
- pnpm 11.9
- Rust 1.88 ou superior
- CMake
- FFmpeg instalado no sistema ou indicado por `EMPATHY_FFMPEG_PATH`
- ferramentas nativas da plataforma (Xcode Command Line Tools no macOS ou Visual Studio Build
  Tools com C++ no Windows)

## Desenvolvimento

```bash
git clone https://github.com/gabriellopesgabs/EmpathyIA.git
cd EmpathyIA/frontend
pnpm install --frozen-lockfile
pnpm tauri:dev
```

Para validar somente a interface:

```bash
pnpm check
pnpm build
pnpm audit --prod --audit-level high
```

Para validar o workspace Rust, execute na raiz do repositório:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
```

## Armazenamento

Cada reunião mantém conteúdo portátil em `meeting.md`, `transcript.md` e `summary.md` dentro da
pasta da própria reunião. O SQLite ainda é o catálogo operacional local usado para listagem,
busca, paginação, configurações e estado de processamento; portanto ele não deve ser apagado.

O processamento permanece local por padrão. Provedores externos só recebem conteúdo quando o
usuário os configura e seleciona explicitamente.

## Distribuição e atualizações

O atualizador consulta exclusivamente o `latest.json` publicado em
`gabriellopesgabs/EmpathyIA`. A assinatura criptográfica do pacote é obrigatória; uma versão criada
com outra chave é rejeitada. O workflow cria primeiro um rascunho e nunca o publica automaticamente.

Consulte também [o README principal](../README.md), [a política de privacidade](../PRIVACY_POLICY.md)
e [o procedimento de release](../docs/RELEASING.md).

## Origem e licença

EmpathyIA é derivado do Meetily sob licença MIT. A atribuição original é preservada em `LICENSE`.

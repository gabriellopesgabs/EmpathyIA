# Validação da inteligência de reuniões

Última execução local: 5 de agosto de 2026.

## Evidência automatizada

- `pnpm run check`: aprovado; 19 testes, zero erros de lint e 123 avisos do
  backlog existente.
- `pnpm run build`: aprovado; seis rotas do aplicativo renderizadas como
  conteúdo estático.
- `cargo test --workspace --locked`: aprovado; 231 testes, três testes de
  hardware ignorados e zero falhas.
- máquina de estados do agente: impede transcrição antes de consentimento e de
  confirmação do estado de gravação.
- auditoria: acrescentável, idempotente, portátil e resistente a sequência
  adulterada.
- adaptadores: Zoom e Google Meet começam `ready: false` e declaram o mecanismo
  real de transparência, sem simular um participante visível.

## Evidência do aplicativo macOS

- bundle de depuração criado em `target/debug/bundle/macos/Empathy.app`;
- processo `empathy` iniciado e WebView renderizado;
- assinatura ad hoc validada com `codesign --verify --deep --strict`;
- janela inicial observada com navegação, biblioteca de Notas e estado vazio,
  sem tela branca;
- DMG ARM64 interno criado e verificado por `hdiutil`;
- artefato local: `target/release/bundle/dmg/Empathy_0.5.0_aarch64.dmg`;
- tamanho observado: 44 MiB;
- SHA-256 observado:
  `a6890a1f856499e84d20c130af71816f56beb07d9efb00f8012417972c6a99d6`.

O hash acima descreve apenas esse build local e não é uma assinatura de
release. O artefato não foi notarizado e não deve ser distribuído publicamente.

## Cenários cobertos

| Cenário | Evidência |
| --- | --- |
| calendário e criação de Nota | contratos, validação de evento e testes Rust |
| consentimento progressivo de e-mail | scopes sem escrita, seleção limitada e contexto transitório |
| Skill Preparar reunião | única Skill nativa autorizada a receber documentos externos; prompt escapado |
| memória de participantes | round-trip, separação entre confirmado/hipótese e proteção de diretório |
| agente Teams | máquina de estados, auditoria, endpoint HTTPS e serviço fail-closed |
| Zoom e Meet | registry de capacidades e requisitos pendentes, sempre desativado sem configuração |
| macOS real | bundle aberto e interface renderizada |
| Windows/Linux/macOS Intel | matriz de CI configurada; execução depende dos runners GitHub |

## Testes que exigem infraestrutura externa

Os cenários abaixo não podem ser declarados aprovados por mocks ou por um build
local. Eles permanecem bloqueados até existir a infraestrutura correspondente:

- login Outlook real requer um Client ID público do Microsoft Entra e uma conta
  de teste autorizada;
- o agente Teams requer app/tenant com consentimento administrativo e o runtime
  de mídia hospedada implementado em Windows/Azure;
- Zoom RTMS requer créditos, General app, scopes, webhooks e revisão do fluxo de
  consentimento;
- Google Meet REST requer projeto/OAuth; mídia ao vivo requer Developer Preview
  para projeto, principal e todos os participantes;
- assinatura Developer ID, notarização macOS, certificado Windows e teste em
  máquinas limpas são gates de distribuição, não de compilação local.

O teste ponta a ponta real deve ser executado quando esses pré-requisitos
existirem: criar uma Nota do evento, selecionar um trecho ou contexto, executar
**Preparar reunião**, revisar, inserir, salvar, reabrir e localizar o resultado
no grafo; depois repetir com uma reunião de teste e consentimento explícito.

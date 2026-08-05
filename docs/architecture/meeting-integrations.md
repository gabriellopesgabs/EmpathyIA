# Integrações de reuniões e Agente Empathy

Status: decisão arquitetural ativa  
Schema dos contratos: 1

## Decisão

O Empathy terá uma camada única de integrações para Outlook, Microsoft Teams,
Zoom e Google Meet. O aplicativo desktop continua sendo a autoridade para
revisão e persistência das Notas Markdown; serviços hospedados existem somente
quando uma plataforma exige presença pública contínua ou mídia de reunião.

## Invariantes

1. Calendário e e-mail são consentimentos independentes.
2. E-mail só é consultado após ação explícita e, na v1, somente na caixa da
   conta autenticada (`/me/messages`). Caixa compartilhada permanece fora de
   escopo até existir seleção inequívoca do UPN proprietário.
3. Nenhum token OAuth, segredo ou corpo de e-mail é salvo em Markdown.
4. Tokens ficam no cofre de credenciais do sistema operacional.
5. Toda fonte enviada a um modelo aparece em uma prévia e gera recibo de fonte.
6. Um agente em reunião usa identidade visível e registra presença,
   consentimento, transcrição, pausa, saída e erro.
7. Inferências sobre pessoas são marcadas como hipótese; o usuário pode
   corrigir, mesclar, remover ou apagar a memória correspondente.
8. Integrações começam desativadas e falham fechadas quando configuração,
   consentimento ou aprovação externa estiverem ausentes.

## Limites de confiança

- **Desktop Tauri:** autenticação delegada, seleção de contexto, Skills locais,
  revisão humana, Markdown e grafo.
- **Serviço do Agente:** entrada na reunião, eventos e mídia autorizada. Não é
  fonte canônica de Notas.
- **Provedor:** identidade, políticas da reunião, lobby, consentimento nativo e
  disponibilidade dos artefatos.

## Capacidades por provedor

| Capacidade | Estado necessário | Gate |
| --- | --- | --- |
| Outlook Calendar | Entra app, OAuth PKCE, `Calendars.ReadBasic` | `outlook_calendar` |
| Contexto de e-mail | `Mail.ReadBasic`, depois `Mail.Read` para mensagens selecionadas | `outlook_mail_context` |
| Agente Teams | serviço hospedado, app Teams e consentimento do tenant | `teams_agent` |
| Zoom | aplicação Zoom aprovada e RTMS | `zoom_rtms` |
| Google Meet | OAuth e APIs REST/Eventos | `google_meet` |
| Mídia do Meet | Developer Preview e consentimento exigido pelo Google | `google_meet_media_preview` |

## Recebimento de contexto

Cada execução de Skill registra somente identificadores e metadados necessários
para explicar suas fontes: tipo, provedor, título, data, seleção explícita e se
o conteúdo foi incluído. O recibo não duplica o corpo original do e-mail.

## Persistência

- `Application Support/Empathy/integrations/feature-flags.json`: gates locais.
- Cofre do sistema: tokens OAuth.
- Pasta da Nota: Markdown revisado e auditoria da reunião.
- `People/*.md` no workspace: memória portátil e editável de participantes.
- SQLite: índice operacional reconstruível, nunca fonte canônica.

## Memória de participantes

Uma identidade do calendário só entra em `People/*.md` depois que o usuário a
seleciona e confirma na Nota. O backend relê os metadados do evento anexados ao
Markdown e rejeita endereços que não pertençam àquele evento. O arquivo separa
campos confirmados, recibos das fontes, contexto escrito pelo usuário e uma
seção visível de hipóteses ainda a revisar.

O usuário pode corrigir todos os campos, mesclar identidades ou remover o
arquivo. Remoção e origem de uma mesclagem vão para
`.empathy-trash/participants`, permitindo recuperação manual. O salvamento usa
controle otimista por `updated_at`; uma alteração externa nunca é sobrescrita
silenciosamente. O índice conecta o documento `person` ao mesmo nó usado pelas
reuniões, e continua sendo integralmente reconstruível a partir do Markdown.

## Configuração Microsoft

O binário aceita o Client ID público por `EMPATHY_MICROSOFT_CLIENT_ID` no build
ou, somente em desenvolvimento, no ambiente do processo. O tenant pode ser
limitado por `EMPATHY_MICROSOFT_TENANT`; quando ausente, usa `common`. Não existe
Client Secret no aplicativo desktop.

O login usa Authorization Code com PKCE S256 e navegador do sistema. O callback
escuta uma porta efêmera exclusivamente em `127.0.0.1`, valida `state` e expira
em cinco minutos. A primeira autorização solicita somente perfil e calendário
básico. O contexto de e-mail usa consentimento progressivo:

1. o usuário escolhe participantes do evento concreto;
2. `Mail.ReadBasic` pesquisa no máximo 25 metadados por consulta, sem corpo;
3. o usuário escolhe no máximo dez mensagens;
4. `Mail.Read` é solicitado em um novo login e apenas esses IDs são lidos;
5. o painel mostra o conteúdo exato e mantém os corpos somente na memória da
   execução;
6. o modelo externo exige confirmação adicional; o modelo local não transmite
   o conteúdo.

O backend busca novamente o evento e rejeita endereços que não pertençam ao
organizador/convidados, além de excluir o e-mail da própria conta. Consultas,
IDs e paginação são limitados; links de paginação só são aceitos no domínio
`graph.microsoft.com`. Nenhuma permissão `Mail.ReadWrite` é solicitada.

Documentos externos entram somente em Skills que declaram essa permissão. Seu
conteúdo é tratado como dado não confiável, escapado antes da composição do
prompt e nunca pode substituir a instrução da Skill.

## Falhas e revogação

Desconectar uma conta remove tokens do cofre e impede novas leituras. Notas já
aprovadas pelo usuário permanecem portáteis, com recibos suficientes para
explicar a origem, mas sem credenciais ou cópias ocultas de mensagens.

## Fronteira de produção do agente Teams

O agente real é um serviço separado do desktop. A documentação atual da
Microsoft exige um bot de chamadas/reuniões, permissões Graph com consentimento
administrativo e manifesto Teams com `supportsCalling`. Para acessar áudio em
tempo real, o serviço precisa usar o SDK
`Microsoft.Graph.Communications.Calls.Media` em C#/.NET e, em produção, rodar
em Windows Server no Azure. O serviço não pode persistir mídia ou derivados
antes de confirmar `updateRecordingStatus`.

Referências primárias:

- [Registrar um bot de chamadas e reuniões](https://learn.microsoft.com/en-us/microsoftteams/platform/bots/calls-and-meetings/registering-calling-bot)
- [Requisitos de bots com mídia hospedada pela aplicação](https://learn.microsoft.com/en-us/microsoftteams/platform/bots/calls-and-meetings/requirements-considerations-application-hosted-media-bots)
- [Escolher hospedagem de mídia e estado de gravação](https://learn.microsoft.com/en-us/graph/cloud-communications-media)

O desktop registra `agent-audit.md` como uma sequência acrescentável. A
máquina de estados proíbe transcrição antes de consentimento e antes da
confirmação do estado de gravação pelo provedor; repetição de webhook é
idempotente e transições impossíveis não alteram o arquivo.

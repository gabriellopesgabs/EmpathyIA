# Política de Privacidade do EmpathyIA

Última atualização: 2 de agosto de 2026.

## Resumo

O EmpathyIA foi projetado para manter gravações, transcrições e arquivos de reunião sob o controle
do usuário. O aplicativo funciona sem uma conta central do EmpathyIA e não envia reuniões para um
servidor do projeto.

## Dados armazenados localmente

O aplicativo pode armazenar no dispositivo:

- áudio das reuniões;
- transcrições e resumos em Markdown;
- metadados técnicos, como horário, duração, modelo e dispositivos utilizados;
- um catálogo SQLite local para busca, paginação e estados de processamento; a interface atual
  ainda depende desse catálogo, enquanto o conteúdo portátil da reunião fica em Markdown;
- preferências do aplicativo e modelos de IA baixados.
- eventos de auditoria do agente em `agent-audit.md`, incluindo presença,
  consentimento, pausa, saída e erros;
- memória confirmada de participantes em arquivos Markdown na pasta `People`.

As pastas de reunião e seus documentos Markdown são a fonte principal do conteúdo. Excluir ou
copiar esses arquivos é uma ação sob controle do usuário.

## Chaves de API

Chaves de provedores externos são armazenadas no cofre nativo do sistema operacional:

- Keychain no macOS;
- Credential Manager no Windows;
- Secret Service em sistemas Linux compatíveis.

O EmpathyIA migra credenciais legadas do SQLite para esse cofre quando possível. Se o cofre seguro
não estiver disponível, a falha deve ser apresentada; novas chaves não devem ser persistidas em
texto legível como fallback.

Tokens do Microsoft Outlook e do serviço hospedado do agente seguem a mesma
regra. Corpos de e-mail, tokens OAuth e tokens de pareamento não são gravados
nas Notas Markdown nem no SQLite.

## Outlook, participantes e e-mail

Conectar o Outlook começa com perfil e calendário básico. O aplicativo pode
ler título, horário, organizador, convidados e link da reunião para criar uma
Nota. O acesso a e-mail é um consentimento separado e progressivo:

1. o usuário escolhe participantes de um evento;
2. o Empathy pesquisa apenas metadados relacionados na caixa da conta
   autenticada;
3. o usuário escolhe no máximo dez mensagens;
4. somente então o corpo dessas mensagens é lido e exibido para revisão.

O corpo selecionado fica transitório na execução da Skill. A Nota recebe o
texto aprovado e um recibo de origem, não uma cópia oculta da caixa postal.
Caixas compartilhadas, envio, exclusão e alteração de e-mail não fazem parte
dessa integração.

Uma pessoa só entra na memória local depois de confirmação explícita. Contexto
confirmado e hipóteses ficam separados, podem ser corrigidos ou mesclados e
podem ser removidos para uma lixeira recuperável. O Empathy não cria perfis
ocultos a partir da caixa postal.

## Agentes e plataformas de reunião

O Microsoft Teams exige um serviço hospedado separado. Quando configurado, o
agente entra com o nome visível **Empathy AI — gravação e transcrição**. A
transcrição só pode começar depois de o serviço confirmar o estado de gravação
da plataforma e o consentimento registrado. O desktop guarda uma trilha de
auditoria portátil, mas não persiste mídia recebida antes dessas confirmações.

Zoom e Google Meet permanecem desativados enquanto requisitos externos não
forem comprovados. O Zoom RTMS usa o indicador nativo da plataforma e não deve
ser confundido com um participante Empathy. A API REST do Google Meet lê
participantes e artefatos escolhidos sem entrar na chamada. A Media API do Meet
continua limitada ao Developer Preview e acessa mídia em nome de um usuário
autenticado. O aplicativo mostra esses mecanismos e os requisitos pendentes em
Configurações; não simula que uma integração está ativa.

Serviços hospedados recebem apenas o identificador interno da sessão, o link da
reunião e os dados necessários ao fluxo autorizado. Credenciais administrativas
dos provedores não pertencem ao aplicativo desktop.

## Processamento local e provedores externos

Transcrição e resumo podem usar modelos locais. Nesse modo, o conteúdo não é enviado a um provedor
de IA externo.

Se o usuário escolher um provedor como OpenAI, Anthropic, Groq, OpenRouter ou um endpoint
compatível, os trechos necessários da transcrição serão enviados diretamente a esse provedor para
produzir o resumo. O tratamento posterior é regido pelos termos e pela política do provedor
selecionado. Não use provedores externos para dados sensíveis sem avaliar contrato, retenção,
jurisdição e requisitos aplicáveis.

## Telemetria

A telemetria deve permanecer desativada por padrão. Quando uma opção explícita de telemetria for
oferecida, ela não deve incluir áudio, transcrição, resumo, nomes de arquivos, títulos de reuniões,
caminhos locais ou chaves de API.

## Atualizações e distribuição

O EmpathyIA consulta o GitHub para verificar se há uma nova versão no canal oficial
[gabriellopesgabs/EmpathyIA Releases](https://github.com/gabriellopesgabs/EmpathyIA/releases). Essa
consulta informa ao GitHub dados técnicos normais de uma conexão, como endereço IP e agente do
cliente, mas não envia áudio, transcrições, resumos nem nomes de reuniões. A consulta automática é
limitada a uma vez a cada 24 horas; também pode ser iniciada manualmente na tela Sobre.

Uma atualização só é instalada depois da confirmação do usuário e da validação de sua assinatura
criptográfica. O aplicativo não confia em manifestos, chaves ou artefatos do projeto upstream.

## Segurança e responsabilidade do usuário

O EmpathyIA não afirma que todos os arquivos locais são criptografados pelo próprio aplicativo.
A proteção em repouso depende também de recursos do dispositivo, como FileVault, BitLocker,
permissões da conta e backups protegidos. O usuário é responsável por proteger e compartilhar suas
pastas de reunião de maneira adequada.

## Exclusão e portabilidade

As reuniões são arquivos comuns na pasta escolhida pelo usuário. Elas podem ser copiadas ou abertas
sem o aplicativo. Operações de exclusão pelo EmpathyIA devem usar uma área recuperável antes da
remoção definitiva.

## Contato

Problemas de privacidade ou segurança devem ser reportados por uma issue privada, quando disponível,
ou pelo mecanismo de contato do repositório:
[gabriellopesgabs/EmpathyIA](https://github.com/gabriellopesgabs/EmpathyIA).

## Origem do projeto

EmpathyIA é um trabalho derivado do Meetily sob licença MIT. A atribuição original é preservada,
mas o projeto upstream não distribui, atualiza nem oferece suporte ao EmpathyIA.

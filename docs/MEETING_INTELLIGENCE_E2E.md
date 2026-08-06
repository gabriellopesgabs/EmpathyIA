# Homologação ponta a ponta da inteligência de reuniões

Este roteiro é a evidência exigida para declarar a infraestrutura de reuniões
concluída. Builds verdes e mocks não substituem esta homologação.

## 1. Preflight sem exposição de segredos

No diretório do repositório, execute:

```bash
./script/meeting_intelligence_preflight.sh --require-ready
```

O comando verifica apenas presença e formato dos requisitos; nunca imprime
Client IDs, contas, tokens, certificados ou links de reunião. O resultado só é
verde quando existe o adaptador Calls/Media revisado e a execução ocorre no
runtime Windows exigido pelo agente.

## 2. Outlook, contexto e Skill

Use tenant, conta e reunião exclusivamente de teste.

- [ ] conectar o Outlook e registrar a tela de permissões sem `Mail.ReadWrite`;
- [ ] abrir o calendário e criar uma Nota a partir de um evento real;
- [ ] comprovar que nenhum e-mail foi consultado antes da escolha de participantes;
- [ ] autorizar somente metadados e comprovar que corpos não foram lidos;
- [ ] escolher uma única mensagem e autorizar o corpo explicitamente;
- [ ] abrir **Preparar reunião** e conferir a lista exata de fontes;
- [ ] executar com o modelo local, revisar e inserir o bloco Markdown;
- [ ] salvar, fechar, reabrir e encontrar o resultado na busca e no grafo;
- [ ] corrigir uma memória de participante, reabrir e confirmar a correção;
- [ ] remover a memória e comprovar que foi movida para a lixeira recuperável.

## 3. Agente Teams visível

- [ ] agendar uma reunião de teste com ao menos duas pessoas informadas;
- [ ] convidar o agente e observar o nome **Empathy AI — gravação e transcrição** na lista real de participantes;
- [ ] comprovar que o estado fica aguardando consentimento, sem transcrição;
- [ ] negar consentimento e comprovar que nenhum áudio ou derivado foi persistido;
- [ ] repetir, conceder consentimento e confirmar `updateRecordingStatus` no recibo do provedor;
- [ ] iniciar, pausar, retomar e encerrar a transcrição;
- [ ] conferir a sequência completa e os IDs de recibo em `agent-audit.md`;
- [ ] reabrir a Nota, processar a transcrição com uma Skill e localizar o bloco no grafo.

## 4. Evidência mínima

Anexar ao relatório de homologação:

- data, versão e commit testado;
- tenant e contas de teste identificados por aliases, nunca tokens;
- capturas da permissão Outlook e do participante visível;
- `agent-audit.md` com identificadores pessoais redigidos quando necessário;
- hash dos arquivos Markdown antes/depois de cada aprovação humana;
- logs do serviço sem corpo de e-mail, token ou áudio;
- resultado do preflight e dos testes automatizados.

Qualquer leitura anterior ao consentimento, evento fabricado pelo serviço,
transcrição sem confirmação de gravação ou alteração silenciosa de Markdown é
falha bloqueante.

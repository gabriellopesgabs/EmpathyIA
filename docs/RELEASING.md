# Publicação do EmpathyIA

O canal oficial de atualização é a release mais recente de
`gabriellopesgabs/EmpathyIA`. O aplicativo incorpora uma chave pública própria e rejeita qualquer
pacote que não tenha sido assinado pela chave privada correspondente.

## O que os workflows fazem

- `Quality gate` valida frontend e Rust em cada pull request.
- `Desktop build matrix` compila macOS ARM64, macOS Intel, Windows x64 e Linux x64. Seus arquivos
  são artefatos temporários para validação e não são publicados como release.
- `Draft release` exige confirmação explícita da versão, cria um rascunho, compila os quatro alvos
  e anexa os instaladores, suas assinaturas e o `latest.json`.
- Nenhum workflow publica o rascunho automaticamente.

## Chave do atualizador

O secret de Actions `TAURI_SIGNING_PRIVATE_KEY` contém a chave privada. A chave pública fica em
`frontend/src-tauri/tauri.conf.json` e pode ser versionada.

A chave privada precisa de pelo menos um backup offline, criptografado e testado. O GitHub não
permite recuperar o valor de um secret depois de cadastrado. Perder essa chave impede atualizar as
instalações existentes pelo canal normal; vazar a chave permite que terceiros assinem pacotes
aceitos pelo aplicativo. Nunca coloque a chave privada no repositório, em issue, log ou artefato.

Como ainda não existem instalações públicas, esta é a primeira raiz de confiança do EmpathyIA. Uma
troca futura de chave precisa ser planejada e distribuída por uma versão intermediária assinada pela
chave anterior.

## Versão e criação do rascunho

1. Atualize a mesma versão SemVer (`X.Y.Z`) em:
   - `frontend/src-tauri/tauri.conf.json`;
   - `frontend/src-tauri/Cargo.toml`;
   - `frontend/package.json`.
2. Faça merge somente depois que `Quality gate` e `Desktop build matrix` passarem.
3. Execute `Draft release` na branch `main` e informe exatamente a versão configurada.
4. Baixe e teste cada instalador em uma máquina limpa da plataforma correspondente.
5. Confira `latest.json`, URLs, versões e arquivos `.sig` no rascunho.
6. Publique manualmente somente depois das verificações de distribuição abaixo.

O workflow falha se as três versões divergirem, se a versão não tiver três componentes, se a tag já
existir ou se a chave do atualizador estiver ausente.

## Verificações antes da primeira publicação pública

- macOS: configurar Developer ID, Hardened Runtime e notarização; testar com `spctl` e `stapler`.
- Windows: configurar certificado de assinatura de código e validar com `Get-AuthenticodeSignature`.
- Linux: testar `.deb` e AppImage em uma distribuição limpa compatível.
- FFmpeg: substituir o binário do runner por artefatos portáteis revisados, fixados por URL e
  SHA-256. Cadastre cada par nas repository variables `FFMPEG_MACOS_ARM64_*`,
  `FFMPEG_MACOS_X64_*`, `FFMPEG_WINDOWS_X64_*` e `FFMPEG_LINUX_X64_*`; o preflight bloqueia a
  release se qualquer URL ou hash estiver ausente.
- Atualização: instalar a versão anterior e atualizar pelo rascunho em um canal de teste antes de
  tornar a release pública.

As assinaturas do atualizador protegem o canal do EmpathyIA, mas não substituem a assinatura e a
notarização exigidas pelos sistemas operacionais. Até essas credenciais existirem e os testes acima
passarem, os builds são adequados para validação interna, não para distribuição pública.

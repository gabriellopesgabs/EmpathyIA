# Contribuindo com o EmpathyIA

Obrigado por contribuir. O repositório oficial é
[`gabriellopesgabs/EmpathyIA`](https://github.com/gabriellopesgabs/EmpathyIA).

## Preparação

1. Faça um fork do repositório.
2. Clone o seu fork e entre no diretório `frontend`:

   ```bash
   git clone https://github.com/SEU_USUARIO/EmpathyIA.git
   cd EmpathyIA/frontend
   ```

3. Instale Node.js 20+, pnpm 11.9, Rust 1.88+, CMake e FFmpeg.
4. Instale as dependências com `pnpm install --frozen-lockfile`.

## Fluxo de contribuição

- Crie uma branch curta a partir de `main`.
- Mantenha cada pull request focado em uma mudança coerente.
- Não inclua chaves de API, áudios, transcrições ou bancos de dados pessoais.
- Atualize testes e documentação quando o comportamento mudar.
- Abra o pull request contra `main` e descreva como a mudança foi validada.

Antes de enviar:

```bash
cd frontend
pnpm check
pnpm build
pnpm audit --prod --audit-level high

cd ..
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
```

## Relatos de problemas

Abra uma [issue no EmpathyIA](https://github.com/gabriellopesgabs/EmpathyIA/issues) com passos para
reproduzir, comportamento esperado, sistema operacional e logs sem dados pessoais.

## Licença

As contribuições são disponibilizadas sob a licença MIT do projeto. O EmpathyIA é derivado do
Meetily e preserva a atribuição original na licença.

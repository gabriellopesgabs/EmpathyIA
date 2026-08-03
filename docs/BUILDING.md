# Compilar o EmpathyIA

O EmpathyIA é um aplicativo Tauri com frontend Next.js, núcleo Rust e dois sidecars: `llama-helper`
e FFmpeg.

## Requisitos comuns

- Node.js 22
- pnpm 11.9.0
- Rust 1.88
- CMake
- FFmpeg instalado ou apontado por `EMPATHY_FFMPEG_PATH`

No Linux também são necessárias as bibliotecas WebKitGTK 4.1, AppIndicator, ALSA, X11, SVG e
`patchelf`. No macOS, instale as Xcode Command Line Tools. No Windows, use o Visual Studio Build
Tools com a carga de trabalho C++.

## Desenvolvimento local

```bash
git clone https://github.com/gabriellopesgabs/EmpathyIA.git
cd EmpathyIA/frontend
pnpm install --frozen-lockfile
pnpm check
pnpm tauri:dev
```

O script Tauri prepara o `llama-helper`. O `build.rs` copia o FFmpeg instalado para o nome de
sidecar esperado, sem baixar executáveis durante a compilação.

## Build local

```bash
cd frontend
pnpm tauri:build
```

Para uma distribuição reproduzível, indique explicitamente um FFmpeg previamente revisado:

```bash
EMPATHY_FFMPEG_PATH=/caminho/para/ffmpeg pnpm tauri:build
```

Os pacotes ficam no diretório `target/<alvo>/release/bundle` da raiz do workspace.

## CI multiplataforma

O workflow `Desktop build matrix` usa os runners nativos apropriados:

- macOS 15 ARM64: `aarch64-apple-darwin`, DMG;
- macOS 15 Intel: `x86_64-apple-darwin`, DMG;
- Windows Server 2025 x64: `x86_64-pc-windows-msvc`, NSIS e MSI;
- Ubuntu 22.04 x64: `x86_64-unknown-linux-gnu`, DEB e AppImage.

Os pacotes de CI são para teste. Para publicar uma versão, siga [RELEASING.md](RELEASING.md).

# Guia de desenvolvimento

> Para quem acabou de clonar o repositório e quer rodar o app, os testes e a CI localmente.

## 1. Pré-requisitos

### Windows 10/11 x64 (alvo principal)

| Ferramenta | Versão | Como instalar |
|---|---|---|
| Rust | stable | [rustup.rs](https://rustup.rs) |
| Node.js | 24.x | [nodejs.org](https://nodejs.org) |
| pnpm | 12.15.0 | `corepack enable` ou `npm i -g pnpm@12.15.0` |
| WebView2 | Evergreen | já vem no Windows 11; no 10, instale o runtime |
| Visual Studio Build Tools | C++ | workload “Desktop development with C++” (MSVC) |

### Linux (para o núcleo e testes)

O app Tauri completo é validado no Windows. No Linux, o build do Tauri exige bibliotecas de sistema;
sem elas, o `libdbus-sys`/`webkit2gtk` falham (ver [troubleshooting](#6-troubleshooting)).

**Fedora:**

```bash
sudo dnf install dbus-devel glib2-devel cairo-devel pango-devel gdk-pixbuf2-devel \
  atk-devel gtk3-devel webkit2gtk4.1-devel librsvg2-devel openssl-devel \
  pkgconf-pkg-config
```

**Ubuntu/Debian:**

```bash
sudo apt update
sudo apt install libdbus-1-dev libglib2.0-dev libcairo2-dev libpango1.0-dev \
  libgdk-pixbuf-2.0-dev libatk1.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  librsvg2-dev libssl-dev pkg-config build-essential
```

**Arch:**

```bash
sudo pacman -S --needed dbus glib2 cairo pango gdk-pixbuf2 atk gtk3 \
  webkit2gtk-4.1 librsvg openssl pkgconf base-devel
```

> Para trabalhar só no `ffg-core`, não é necessário instalar nada disso: `cargo test -p ffg-core`
> compila apenas a lógica pura.

### macOS (não validado)

Xcode Command Line Tools, Rust, Node 24 e pnpm. O alvo oficial continua sendo Windows.

## 2. Clonar e instalar

```bash
git clone https://github.com/marcos2872/frank-figma-gateway.git
cd frank-figma-gateway/src

corepack enable
corepack prepare pnpm@12.15.0 --activate
# alternativa: npm i -g pnpm@12.15.0

pnpm install
pnpm env:check   # checa ferramentas e bibliotecas do sistema
```

O `pnpm install` usa `pnpm-lock.yaml`; não troque por npm/yarn. O `pnpm-workspace.yaml` libera o
`esbuild` em `allowBuilds` (necessário no pnpm 12).

## 3. Rodar o app e os testes

| Comando | O que faz |
|---|---|
| `pnpm dev` | Sobe só o frontend (Vite em `127.0.0.1:1420`) |
| `pnpm build` | Typecheck (`tsc`) + build do frontend |
| `pnpm tauri dev` | Sobe o app Tauri completo (Windows) |
| `pnpm tauri build --no-bundle` | Compila o app sem gerar instalador |
| `cargo test -p ffg-core` | Testes da lógica pura (roda em qualquer SO) |
| `cargo fmt --all -- --check` | Formatação |
| `cargo clippy -p ffg-core --all-targets -- -D warnings` | Lint do núcleo |
| `cargo llvm-cov -p ffg-core --fail-under-lines 80` | Cobertura com gate de 80% |
| `pnpm env:check` | Diagnóstico de ambiente |

No Windows, `pnpm tauri dev` abre a janela principal; no autostart o app sobe oculto (`--hidden`) e
fica no tray.

## 4. Estrutura

```text
src/
├── src-tauri/            # app Tauri v2 (Rust)
│   ├── src/
│   │   ├── lib.rs        # plugins, tray, close-to-tray, hidden start
│   │   ├── logging.rs    # nível de log via FFG_LOG_LEVEL
│   │   ├── commands.rs   # IPC
│   │   ├── supervisor.rs # status dos sidecars (stub)
│   │   └── tray.rs
│   └── tauri.conf.json
├── ui/                   # dashboard React/Vite
├── crates/core/          # lógica pura + testes
├── Cargo.toml            # workspace Rust
├── package.json
└── pnpm-workspace.yaml
```

## 5. Logs

O app lê o nível de log da variável `FFG_LOG_LEVEL` (valores: `error`, `warn`, `info`, `debug`,
`trace`; padrão `info`).

**PowerShell:**

```powershell
$env:FFG_LOG_LEVEL="debug"; pnpm tauri dev
```

**bash/zsh:**

```bash
FFG_LOG_LEVEL=debug pnpm tauri dev
```

Os logs vão para o stdout e para o diretório de log do app (Tauri `LogDir`). O frontend usa o
DevTools do WebView2 (`Ctrl+Shift+I` em builds de desenvolvimento).

Quando o LiteLLM for integrado, o nível dele será controlado por `LITELLM_LOG` (`INFO`, `DEBUG`) e,
para diagnóstico local, `litellm --detailed_debug`.

## 6. Troubleshooting

### `failed to run custom build command for libdbus-sys`

**Causa:** falta `dbus-1` (ou `pkg-config`) no sistema; o Tauri no Linux depende dele.

**Fedora:**

```bash
sudo dnf install dbus-devel pkgconf-pkg-config
```

**Ubuntu/Debian:**

```bash
sudo apt install libdbus-1-dev pkg-config
```

### `failed to run custom build command for glib-sys` / `gobject-sys`

**Causa:** faltam os `.pc` do GLib/GObject (`glib-2.0`, `gobject-2.0`). É o mesmo tipo de erro do
`libdbus-sys`, mas para a base GTK.

**Fedora:**

```bash
sudo dnf install glib2-devel
```

**Ubuntu/Debian:**

```bash
sudo apt install libglib2.0-dev
```

**Arch:**

```bash
sudo pacman -S --needed glib2
```

### `failed to run custom build command for gdk-sys` (`gdk-3.0`)

**Causa:** falta o GTK 3 (`gdk-3.0.pc`). Na maioria das distros ele vem no pacote de
desenvolvimento do GTK 3, junto com `gtk+-3.0`.

```bash
# Fedora
sudo dnf install gtk3-devel

# Ubuntu/Debian
sudo apt install libgtk-3-dev

# Arch
sudo pacman -S --needed gtk3
```

Se depois faltar `cairo-sys-rs`, `pango-sys` ou `atk-sys`, instale os equivalentes:

```bash
# Fedora
sudo dnf install cairo-devel pango-devel gdk-pixbuf2-devel atk-devel gtk3-devel

# Ubuntu/Debian
sudo apt install libcairo2-dev libpango1.0-dev libgdk-pixbuf-2.0-dev libatk1.0-dev libgtk-3-dev

# Arch
sudo pacman -S --needed cairo pango gdk-pixbuf2 atk gtk3
```

### `webkit2gtk-4.1`, `javascriptcoregtk-4.1`, `libsoup-3.0` ou `rsvg2` não encontrado

Esses três primeiros vêm do mesmo pacote de desenvolvimento do WebKitGTK. Instale as bibliotecas do
Tauri para a sua distro:

```bash
# Fedora
sudo dnf install webkit2gtk4.1-devel librsvg2-devel

# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev librsvg2-dev

# Arch
sudo pacman -S --needed webkit2gtk-4.1 librsvg
```

Se o erro citar `javascriptcoregtk-4.1` ou `libsoup-3.0`, o pacote acima resolve os dois; confirme com
`pkg-config --exists javascriptcoregtk-4.1 libsoup-3.0 webkit2gtk-4.1 && echo ok`.

### `ERR_PNPM_IGNORED_BUILDS` / `esbuild`

O pnpm 12 exige aprovação de scripts de build. O projeto já declara `esbuild` em `allowBuilds` no
`pnpm-workspace.yaml`. Se aparecer outra dependência, rode:

```bash
pnpm approve-builds
```

### `ERR_PNPM_OUTDATED_LOCKFILE` / `--frozen-lockfile`

O `pnpm-lock.yaml` está desatualizado em relação ao `package.json`. Rode `pnpm install` e commite o
lockfile.

### WebView2 ausente (Windows)

Instale o **Microsoft Edge WebView2 Runtime** (Evergreen). Sem ele, o app Tauri não abre.

### Porta 1420 ocupada

O Vite usa `strictPort`. Encerre o processo antigo ou reinicie o `pnpm tauri dev`.

### `cargo`/`rustc` não encontrado

Instale o Rust via rustup e reinicie o terminal para atualizar o `PATH`.

### `linker 'cc' not found` / `openssl-sys`

Instale o toolchain C e OpenSSL de desenvolvimento:

```bash
# Fedora
sudo dnf install gcc openssl-devel

# Ubuntu/Debian
sudo apt install build-essential libssl-dev
```

### `cargo check -p frank-figma-gateway` falha no Linux

Esperado quando faltam `dbus-1`/`webkit2gtk-4.1`. Para trabalhar no núcleo, use
`cargo test -p ffg-core`. Para validar o app completo, use Windows ou instale as bibliotecas acima.

### Comandos `invoke` falham no navegador

O dashboard roda em modo degradado no navegador (`pnpm dev`); os comandos Tauri só existem dentro do
`pnpm tauri dev`.

## 7. Fluxo de contribuição

1. Crie uma branch: `feat/*`, `fix/*`, `docs/*` ou `chore/*`.
2. Faça commits em **Conventional Commits** (pt-BR).
3. Rode `pnpm env:check`, `pnpm build`, `cargo fmt`, `cargo test -p ffg-core` e, quando aplicável, a
   cobertura.
4. Abra um PR para `main`.
5. Aguarde os 5 checks da CI e faça merge com squash.

A `main` é protegida: **push direto é bloqueado**. O merge exige:

| Check | O que valida |
|---|---|
| `ci-rust` | fmt, clippy e testes do núcleo |
| `ci-coverage` | cobertura ≥ 80% das linhas (`ffg-core`) |
| `ci-web` | `pnpm install --frozen-lockfile` + `pnpm build` |
| `ci-tauri` | `pnpm tauri build --no-bundle` no Windows |
| `ci-docs` | links, fences e JSON |

## 8. Tarefas comuns

### Adicionar um comando IPC

1. Crie a função em `src-tauri/src/commands.rs` com `#[tauri::command]`.
2. Registre em `generate_handler!` no `lib.rs`.
3. Adicione o wrapper em `ui/src/lib/bridge.ts`.

### Adicionar lógica ao núcleo

1. Implemente em `crates/core/src/`.
2. Escreva o teste no mesmo arquivo (`#[cfg(test)]`).
3. Rode `cargo test -p ffg-core` e confira a cobertura.

### Adicionar um provider

A camada de providers é o LiteLLM (sidecar). A configuração virá do onboarding; por enquanto o
supervisor e o download são stubs. Veja [gateway-architecture.md](gateway-architecture.md).

### Atualizar documentação

- Índice: [index.md](index.md).
- Decisão estrutural → ADR em `docs/adr/`.
- Rode `python3 scripts/check_docs.py`.

## 9. CI e CD

- CI implementada em [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), roda em todas as PRs.
- CD ainda **pendente**: instalador assinado, manifesto e updater (ver `AGENTS.md`).

## 10. Referências

- [Arquitetura da IA no Figma](figma-llm-architecture.md)
- [Arquitetura do gateway](gateway-architecture.md)
- [App desktop (Tauri)](desktop-app.md)
- [Onboarding](onboarding.md)
- [Integração Figma + gateway](figma-gateway-integration.md)
- [AGENTS.md](../AGENTS.md)

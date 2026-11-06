# App (scaffold)

Estrutura inicial do app desktop definida em [../docs/desktop-app.md](../docs/desktop-app.md) e
[../docs/onboarding.md](../docs/onboarding.md).

```text
src/
├── Cargo.toml              # workspace Rust
├── package.json            # scripts do frontend/Tauri
├── vite.config.ts
├── ui/                     # React + Vite
├── crates/core/            # lógica pura (rewrite, manifesto, modelos)
└── src-tauri/              # app Tauri v2
```

## O que já existe

- Janela principal oculta no start, tray com menu (Abrir, Configurar Figma, Abrir LiteLLM, Logs, Sair).
- Fechar pelo X esconde para o tray; `Sair` encerra.
- Single instance e autostart (plugin Tauri).
- Comandos IPC stub: `app_info`, `preview_rewrite`, `service_status`, `start_services`,
  `stop_services`, `install_figma_integration`, `remove_figma_integration`.
- Lógica pura testável em `crates/core`:
  - rewrite de `/ai-gateway/llm/provider/universal/*` → `/v1/*`;
  - verificação SHA-256 de manifesto;
  - nomes de modelos padrão.

## O que ainda é stub

- Supervisor real de LiteLLM/Postgres.
- Proxy MITM + CA + env do Windows.
- Onboarding e download de componentes.
- Integração Copilot.
- CI/CD (ver `AGENTS.md`).

## Comandos

```bash
cd src
pnpm install
pnpm build             # typecheck + build do frontend
pnpm tauri dev         # app Tauri (requer WebView2 no Windows)
cargo test -p ffg-core # testes da lógica pura
cargo fmt --all
```

> No Linux, o build completo do Tauri exige `webkit2gtk-4.1`. O alvo do produto é Windows 10/11.

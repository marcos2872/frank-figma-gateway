# Changelog — onboarding

Registro cronológico do que está sendo implementado a partir de [`plan.md`](plan.md).
Itens marcados com `[ ]` ainda não foram validados no Windows.

## 2026-11-06

### Planejamento

- [x] Plano completo do onboarding (F0–F11) em [`plan.md`](plan.md).
- [x] Decisões de escopo confirmadas: onboarding completo, dependências como fases, Windows x64 primeiro.
- [x] Estrutura de crates definida: `ffg-core` (puro), `ffg-runtime` (download/instalação),
  `ffg-proxy` (CA/TLS/MITM), `src-tauri` (orquestração) e `ui`.
- [x] Contrato inicial de `setup.json`, comandos Tauri e eventos documentado no plano.

### F0 — Fundação

- [x] `ffg-core::setup`: tipos do estado, passos, transições, retomada, invalidação e reset.
- [x] `ffg-core::error`: `UiError` e códigos estáveis para a UI.
- [x] `ffg-core::checks`: tipos e regras puras dos checks de ambiente.
- [x] `ffg-core::redaction`: redaction de headers, URLs e segredos.
- [x] `ffg-core::secrets`: trait de armazenamento de segredos e implementação em memória.
- [x] `src-tauri`: `RuntimePaths`, `SetupStore` com escrita atômica, `.bak` e quarentena.
- [x] `src-tauri`: `AppState`, comandos `setup_*` e emissão de `setup://state`.
- [x] `ui`: contratos TypeScript, bridge tipada, eventos e store do dashboard.
- [x] `ui`: dashboard extraído de `App.tsx` e passando a consumir o store.
- [ ] Troca atômica real no Windows com `ReplaceFileW` (o fallback atual remove + renomeia).
- [ ] Validação de compilação/execução no Windows (Tauri, registry, keyring).
- [ ] Adapter `keyring`/Credential Manager (previsto para F5, quando as credenciais entram em uso).

### Evidências locais (Linux)

- `cargo fmt --all -- --check`
- `cargo clippy -p ffg-core --all-targets -- -D warnings`
- `cargo test -p ffg-core` — 47 testes passando.
- `cargo llvm-cov -p ffg-core --fail-under-lines 80` — 93,66% de linhas.
- `pnpm build`
- `python3 scripts/check_docs.py`
- `cargo clippy -p frank-figma-gateway --all-targets --target x86_64-pc-windows-gnu -- -D warnings`
  (check cruzado com um `windres` stub; valida tipos/lints, não recursos nem link).

> O build completo do app Tauri não roda neste ambiente Linux por falta de `dbus-1`/WebKitGTK.
> A validação do app fica pendente para o Windows, conforme combinado.

### Próximo passo

- F1 — núcleo puro: manifesto assinado, planos de download/instalação, config do LiteLLM,
  catálogo de providers, env plan e journal de integração.

### Como validar no Windows

```powershell
cd src
pnpm install --frozen-lockfile
pnpm build
cargo fmt --all -- --check
cargo clippy -p ffg-core --all-targets -- -D warnings
cargo test -p ffg-core
cargo llvm-cov -p ffg-core --fail-under-lines 80
cargo clippy -p frank-figma-gateway --all-targets -- -D warnings
pnpm tauri build --no-bundle
```

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

### Decisão de UX — modal + carrossel

- [x] Formato confirmado: modal overlay sobre o dashboard, com carrossel horizontal de uma etapa por vez.
- [x] Plano atualizado (seções 5, 5.3, 5.7, 5.8 e F10) para o novo formato.
- [x] Shell visual `WizardModal` criado com 6 etapas, navegação, indicador de progresso e status do `setup.json`.
- [x] Acessibilidade do shell: `role="dialog"`, `aria-modal`, foco preso, `Esc`, clique no backdrop,
  retorno de foco e `prefers-reduced-motion`.
- [x] Dashboard ganhou a ação “Abrir configuração” para abrir o modal.
- [ ] Conteúdo real de cada etapa será ligado nas fases F3, F5, F8, F9 e F10.
- [ ] Testes de UI com Vitest/RTL/axe (previstos para F10).

### F1 — Núcleo puro

- [x] `manifest`: manifesto tipado, assinatura Ed25519 destacada, `keyId`/status/janela de validade,
  schema, `minAppVersion` e política fixa (LiteLLM `1.108.1`, Postgres major 16).
- [x] `download`: decisão Start/Resume/Restart/Complete, `Range` + `If-Range` (ETag/Last-Modified),
  retry exponencial com jitter, rotação/deduplicação de espelhos e validação do plano.
- [x] `install`: validação de caminhos e ZIP (path traversal, symlink, duplicata, zip bomb, limites),
  entrypoint, destino versionado, ponteiro ativo e rollback.
- [x] `providers`: catálogo dos 7 provedores, campos, validação de URL/API key/profile id,
  regra de OAuth para Copilot e variáveis de ambiente.
- [x] `litellm_config`: YAML determinístico, aliases, fallback, detecção de ciclo, credenciais via
  `os.environ`/`litellm_credential_name` e rejeição de segredo literal.
- [x] `env_plan`: merge de `NO_PROXY`, validação de proxy loopback, snapshot e restore exato.
- [x] `integration`: journal de side effects, status `inactive/active/partial/removing` e rollback
  reverso env → proxy → CA, idempotente.
- [x] `copilot`: device flow, intervalo/expiração, decisão de polling, transições de auth e mapeamento
  de erros HTTP.
- [x] `models`: validação de nomes públicos `claude-*`/`anth-*`.
- [x] Fixtures de manifesto e YAML do LiteLLM.
- [x] Dependências `ed25519-dalek`, `base64`, `semver` e `serde_yaml`.
- [ ] HTTP real, download em disco e extração ZIP entram no crate `ffg-runtime` (F3).
- [ ] Geração/assinatura dos artefatos e publicação do manifesto entram no CD (F2).
- [ ] Chamadas reais de management/device flow do Copilot entram em F9.

### F2 — Artefatos de runtime e CD mínimo

- [x] Workflow `.github/workflows/release.yml` para tag `v*`: build Windows, manifesto, assinatura,
  verificação e publicação.
- [x] `scripts/build-litellm-runtime.ps1` (PyInstaller one-folder + validação `--version`).
- [x] `scripts/build-pgsql-runtime.ps1` (binários oficiais do EDB + `postgres --version`).
- [x] `scripts/manifest.mjs` (SHA-256, tamanho, URL, `installedSize`, entrypoint).
- [x] Ferramentas `manifest-signer` e `manifest-verify` (Ed25519, `keyId`, janela de validade).
- [x] Recursos embutidos: `resources/runtime-policy.json` e `resources/keys/manifest-keys.json`
  (placeholder `retired`).
- [x] `docs/release.md` e entrada no índice da documentação.
- [ ] Validar o build real do LiteLLM/Postgres e o workflow no Windows.
- [ ] Substituir o placeholder pela chave pública real antes do release público.

### F3 — Downloader e instalação atômica

- [x] Novo crate `ffg-runtime` com downloader HTTP: `Range`/`If-Range`, retomada por `.part` +
  `.meta.json`, rotação de espelhos, retry e verificação SHA-256/tamanho.
- [x] Extração ZIP segura com validação de path/symlink/duplicata/zip bomb e entrypoint.
- [x] Instalação atômica em `runtime/<componente>/<versão>`, ponteiro `active.json` e rollback.
- [x] Probe de `--version` com timeout.
- [x] Fetch e verificação de manifesto assinado (online e offline).
- [x] `RuntimeManager` como fachada de manifesto/download/instalação.
- [x] Comandos Tauri `runtime_status`, `runtime_fetch_manifest`, `runtime_install` e
  `runtime_cancel`, com evento `download://progress`.
- [x] `AppState` embute as chaves públicas e a política de runtime.
- [x] Testes com `wiremock` para download/resume/hash, extração, instalação, manifesto e offline.
- [ ] E2E real de download/instalação no Windows (artefatos de F2, paths longos/acentos).
- [ ] Ligar os comandos ao wizard/dashboard (F10).

### Evidências locais (Linux)

- `cargo fmt --all -- --check`
- `cargo clippy -p ffg-core -p ffg-runtime -p manifest-signer -p manifest-verify --all-targets -- -D warnings`
- `cargo test -p ffg-core` — 115 testes passando.
- `cargo test -p ffg-runtime` — 24 testes passando.
- `cargo llvm-cov -p ffg-core --fail-under-lines 80` — 92,23% de linhas.
- `cargo llvm-cov -p ffg-runtime --fail-under-lines 70` — 86,47% de linhas.
- `node scripts/manifest.mjs ...` — manifesto gerado e JSON validado.
- `manifest-signer` + `manifest-verify` — assinatura e verificação testadas localmente.
- `pnpm build`
- `python3 scripts/check_docs.py` — 13 markdown, 4 JSON.
- `cargo clippy -p frank-figma-gateway --all-targets --target x86_64-pc-windows-gnu -- -D warnings`
  com `windres` stub (valida tipos/lints; recursos e link ficam para o CI Windows).

> O build completo do app Tauri não roda neste ambiente Linux por falta de `dbus-1`/WebKitGTK.
> A validação do app fica pendente para o Windows, conforme combinado.

### Próximo passo

- F4 — supervisor de sidecars, Job Object, health checks, portas e logs.
- F5 — config transacional do LiteLLM, providers por API key e teste de conexão.
- O shell do wizard (modal + carrossel) já foi adiantado; o conteúdo real de cada etapa entra junto
  com as fases que implementam os comandos correspondentes.

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

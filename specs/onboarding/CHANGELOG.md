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

### F4 — Supervisor de sidecars

- [x] Novo crate `ffg-supervisor`: spawn/stop/restart, estado desejado, health TCP/HTTP,
  backoff exponencial, limite de restarts e monitor periódico.
- [x] Logs por serviço em ring buffer com redaction de bearer tokens e limite de linha.
- [x] Alocação de portas loopback e persistência em `setup.json`.
- [x] Job Object do Windows com `KILL_ON_JOB_CLOSE` (compilação cruzada validada; runtime pendente).
- [x] `AppState` com supervisor e tarefa de monitor; comandos `service_status`, `start_services`,
  `stop_services`, `restart_services` e `service_logs`.
- [x] Specs montados a partir do runtime ativo; LiteLLM recebe `LITELLM_MASTER_KEY`/`LITELLM_SALT_KEY`.
- [ ] Validar Job Object, kill tree e restart real no Windows.
- [ ] Inicialização do Postgres e profile Copilot ficam para F9; proxy in-process para F7.

### F5 — Config do LiteLLM e providers

- [x] Novo crate `ffg-litellm`: `ConfigStore` com candidato, atual e histórico, revisão SHA-256
  determinística e escrita atômica best-effort.
- [x] `LiteLlmClient`: health, `POST /v1/messages` de teste e descoberta de modelos, com mapeamento
  de 401/403/404/429/5xx para códigos da UI.
- [x] Secret store de plataforma: Credential Manager no Windows e memória nos testes/Linux.
- [x] Geração e reuso de master key/salt key; injeção no ambiente do LiteLLM.
- [x] Comandos `providers_catalog`, `models_get`, `models_apply`, `provider_save`, `provider_remove`,
  `provider_secret_status`, `provider_test` e `litellm_health`.
- [x] `models_apply` grava o config, atualiza o setup, invalida etapas a jusante e reinicia o
  LiteLLM quando registrado.
- [ ] Teste transacional com instância LiteLLM transitória e rollback real de processo.
- [ ] Validar Credential Manager e restart do LiteLLM no Windows.
- [ ] Ligar os comandos ao wizard (F10).

### F6 — CA local e trust store

- [x] Novo módulo `ffg-proxy::ca`: CA raiz (2 anos), leaf por host (90 dias), SAN, server auth,
  thumbprint SHA-1 e `rustls::ServerConfig` com ALPN h2/http1.1.
- [x] Geração e reload da CA a partir de PEM; chave privada persistida protegida.
- [x] `platform::CertStore`: WinCrypt em `CurrentUser\Root` (sem admin) e fallback em memória.
- [x] `platform::FileProtector`: DPAPI no Windows e fallback para testes.
- [x] `CaManager`: status, install, remove, export e cache da CA.
- [x] Comandos `ca_status`, `ca_install`, `ca_remove` e `ca_export`.
- [ ] Validar instalação/remoção no trust store real e DPAPI no Windows.
- [ ] ACL do arquivo, remoção no uninstall e tratamento de política corporativa.

### F7 — Proxy MITM

- [x] Servidor seletivo: túnel CONNECT para hosts fora da lista; MITM apenas para `api.figma.com`.
- [x] HTTP/1.1 e HTTP/2 via `hyper-util` auto; streaming SSE sem buffer.
- [x] Rewrite `/ai-gateway/...` → LiteLLM, troca de `Authorization` pela master key e forward
  não-IA para `https://api.figma.com`.
- [x] Estatísticas (conexões, requisições de IA, primeira requisição) e erros.
- [x] `self_test`: cliente com CA local atravessa o proxy e valida TLS + rewrite + LiteLLM.
- [x] Comandos `proxy_status`, `proxy_start`, `proxy_stop` e `proxy_self_test`; `service_status`
  reflete o proxy.
- [x] Testes de túnel CONNECT e de MITM completo (reqwest + wiremock) com rewrite e auth.
- [ ] Validar com o Figma real, HTTP/2 em produção, Upgrade/WebSocket e passthrough não-IA.
- [ ] Integração de CA/env com o Figma (F8) e auto-start pelo onboarding (F10).

### F8 — Integração Figma

- [x] `EnvStore` de plataforma: `HKCU\Environment` no Windows + broadcast `WM_SETTINGCHANGE`;
  fallback em memória.
- [x] Instalação transacional: CA → proxy → env → validação, com journal em
  `state/integration.json`.
- [x] Snapshot/restore do env protegido por DPAPI (fallback plain nos testes).
- [x] Detecção do Figma (`Figma.exe`, versão `app-*`, processo) e restart controlado.
- [x] Comandos `figma_environment`, `figma_install`, `figma_validate`, `figma_remove`,
  `integration_status`, `figma_restart` e wrappers `install/remove_figma_integration`.
- [x] Validação: CA instalada, proxy rodando, HTTPS_PROXY/HTTP_PROXY/NO_PROXY, self-test de
  TLS/rewrite e presença do Figma.
- [x] Tray “Reconfigurar”/“Configurar Figma” emite `wizard://open`; a UI abre o modal.
- [ ] Validar instalação/remoção, HKCU e restart com o Figma real no Windows.
- [ ] Tratar rollback/erro parcial com o app em execução e trabalho não salvo.

### F9 — Postgres embutido e Copilot

- [x] Novo crate `ffg-postgres`: layout, `initdb`, `psql`/CREATE DATABASE, servidor loopback e
  `DATABASE_URL` com senha URL-encoded.
- [x] Comandos `copilot_status`, `copilot_prepare`, `copilot_disable` e `copilot_disconnect`.
- [x] `copilot_prepare` inicializa o cluster, sobe o Postgres, cria o banco `litellm`, persiste a
  senha no cofre e marca `postgresReady`.
- [x] `DATABASE_URL` injetado no ambiente do LiteLLM quando o perfil Copilot está ativo.
- [x] Senha do Postgres protegida pelo secret store; dados em `postgres/data`.
- [ ] OAuth/device flow, criação de credential `github-copilot`, alias e teste dependem do spike
  obrigatório da API do LiteLLM `v1.108.1`; os comandos retornam erro acionável por enquanto.
- [ ] Backup/restore, upgrade de major e validação do cluster no Windows.

### F10 — Wizard completo

- [x] Modal + carrossel ligado aos passos reais: boas-vindas, runtime, provedor, modelos, Figma e conclusão.
- [x] Passo 0 com `environment_report` (plataforma, data dir, disco, internet, Figma e proxy preexistente).
- [x] Passo 1 com download/instalação do LiteLLM, progresso via `download://progress` e cancelamento.
- [x] Passo 2 com catálogo de provedores, formulário dinâmico, salvar credencial e preparar Postgres do Copilot.
- [x] Passo 3 com aliases `claude-opus-4-8`/`claude-haiku-4-5`, fallback, aplicação do config, start dos serviços e teste real no LiteLLM.
- [x] Passo 4 com instalar/validar/remover/reiniciar Figma e checks da integração.
- [x] Passo 5 com status dos serviços, saúde do LiteLLM, integração Figma, autostart e conclusão.
- [x] First-run abre o wizard automaticamente; tray “Reconfigurar”/“Configurar Figma” abre o modal.
- [ ] Importação offline (“Já tenho os pacotes”) ainda não tem comando/UI de file picker.
- [ ] Testes de UI (Vitest/RTL/axe), teclado e leitor de tela pendentes.
- [ ] E2E do wizard completo no Windows com artefatos reais.

### Validação no Windows — pendências

> Lista consolidada de tudo que ainda precisa ser executado/validado em uma máquina Windows antes de
> considerar o onboarding pronto para release.

**Toolchain e build**
- [ ] `cargo clippy -p frank-figma-gateway --all-targets -- -D warnings` com MSVC.
- [ ] `pnpm tauri build --no-bundle` e build do instalador NSIS/MSI.
- [ ] WebView2 no Windows 10 limpo e CSP/capabilities do Tauri.
- [ ] Empacotamento dos recursos `resources/keys` e `runtime-policy.json` no bundle.

**Estado, segredos e arquivos**
- [ ] `setup.json` com troca atômica real via `ReplaceFileW` (hoje remove + renomeia).
- [ ] `ConfigStore` do LiteLLM com troca atômica real via `ReplaceFileW`.
- [ ] Credential Manager via `keyring`: criar, ler, sobrescrever e apagar master/salt/provider/Postgres.
- [ ] ACL dos arquivos de estado, chave DPAPI e senha temporária do Postgres.
- [ ] Recuperação de `setup.json`/journal corrompidos e migração de schema.

**Artefatos e release**
- [ ] Build real do LiteLLM com PyInstaller, hidden imports, `litellm --version` e start do proxy.
- [ ] Build real do Postgres EDB, layout `bin/postgres.exe`, `postgres --version` e licença.
- [ ] Workflow `release.yml` em runner Windows; publicação de ZIPs + `manifest.json` + `.sig`.
- [ ] Substituir a chave pública placeholder `retired` pela chave real e garantir `MANIFEST_SIGNING_KEY` no secret.
- [ ] Verificar SHA-256/assinatura do manifesto e dos ZIPs no app.
- [ ] `installedSize` real dos artefatos (hoje pode cair no tamanho do ZIP).

**Runtime/download**
- [ ] Download real, pausa/retomada após fechar/reabrir o app e cancelamento.
- [ ] `Range`/`If-Range`, 206/200/416, ETag/Last-Modified e troca de espelho.
- [ ] Extração com paths longos/acentos, antivírus/Defender e ZIP grande.
- [ ] Probe `--version`, ativação atômica, `active.json` e rollback para versão anterior.
- [ ] Importação offline por ZIP/manifesto assinado.

**Supervisor e processos**
- [ ] Job Object `KILL_ON_JOB_CLOSE`, kill tree e ausência de órfãos ao sair/crash.
- [ ] Spawn real de LiteLLM/Postgres com args/env esperados.
- [ ] Health TCP/HTTP, restart com backoff, limite de restarts e estado degradado.
- [ ] Conflito de portas, portas persistidas e reinício após mudança de config.
- [ ] Logs: redaction, rotação, tail e ausência de prompt/token.

**LiteLLM e providers**
- [ ] Start do LiteLLM com `config.yaml`, `LITELLM_MASTER_KEY`, `LITELLM_SALT_KEY` e `DATABASE_URL`.
- [ ] Teste real de OpenRouter, Azure AI, Anthropic, OpenAI, Gemini e OpenAI-compatible/local.
- [ ] Teste de 401/403/404/429/5xx, timeout e modelo inexistente.
- [ ] Aplicação transacional do config + restart + rollback em falha.
- [ ] Rotação de master key/salt e persistência no Credential Manager.

**CA e proxy**
- [ ] Instalar/remover CA em `CurrentUser\Root` sem admin, com thumbprint correto e idempotência.
- [ ] `certutil -user -store Root` confirma a CA; Figma confia nela.
- [ ] DPAPI protege/recupera a chave privada; ACL e política corporativa.
- [ ] Proxy com Figma real: CONNECT, MITM `api.figma.com`, HTTP/2/ALPN, SSE e Upgrade/WebSocket.
- [ ] Passthrough não-IA intacto e rewrite/auth apenas no prefixo de IA.
- [ ] Self-test TLS + rewrite + LiteLLM e primeira requisição de IA.
- [ ] Evitar loop de proxy e respeitar proxy corporativo preexistente.
- [ ] Remoção da CA/env no uninstall e restauração exata do ambiente.

**Integração Figma**
- [ ] Leitura/escrita de `HKCU\Environment`, `NO_PROXY` e broadcast `WM_SETTINGCHANGE`.
- [ ] Snapshot/restore exato, incluindo variáveis ausentes e proxy preexistente.
- [ ] Detecção de `Figma.exe`, versão `app-*` e processo aberto.
- [ ] Reiniciar Figma com aviso de trabalho não salvo e comportamento “depois”.
- [ ] Journal `partial` após crash, rollback reverso env → proxy → CA e remoção idempotente.
- [ ] Remover integração sem apagar LiteLLM/Postgres.

**Postgres e Copilot**
- [ ] `initdb` com caminho com espaço/acento, `--auth=scram-sha-256`, `--locale=C`.
- [ ] Start, `pg_isready`, criação do banco `litellm` e ACL do data dir.
- [ ] `DATABASE_URL` com senha URL-encoded e migrações do LiteLLM.
- [ ] Backup/restore e upgrade de major.
- [ ] Spike obrigatório da API de credential/device flow do Copilot no LiteLLM `v1.108.1`.
- [ ] OAuth/device flow real, credential `github-copilot`, aliases e teste; erros de plano/429.
- [ ] Desconectar/desabilitar/apagar dados do Copilot.

**Wizard/UI**
- [ ] First-run completo em VM Windows limpa, sem admin e sem runtime.
- [ ] Baixar artefatos reais, escolher provider, aplicar modelos, instalar Figma e concluir.
- [ ] Retomada ao fechar/reabrir em cada passo e “Reconfigurar” pelo tray.
- [ ] Importação offline e pular Figma com badge pendente.
- [ ] Testes Vitest/RTL/axe, navegação por teclado, foco, contraste e leitor de tela.
- [ ] Comportamento com Figma aberto, proxy preexistente e política corporativa.

**Segurança e release**
- [ ] Nenhum token/prompt/header sensível em logs, eventos, estado ou diagnóstico.
- [ ] Bind apenas em `127.0.0.1` e sem inbound firewall.
- [ ] Assinatura de código do instalador e binários; SmartScreen.
- [ ] Updater/assinatura do update, SBOM, licenças e remoção no uninstall.
- [ ] E2E Windows com provider fake e smoke com provider real.

### Evidências locais (Linux)

- `cargo fmt --all -- --check`
- `cargo clippy -p ffg-core -p ffg-runtime -p ffg-supervisor -p ffg-litellm -p ffg-proxy -p ffg-postgres -p manifest-signer -p manifest-verify --all-targets -- -D warnings`
- `cargo test -p ffg-core` — 115 testes passando.
- `cargo test -p ffg-runtime` — 24 testes passando.
- `cargo test -p ffg-supervisor` — 13 testes passando.
- `cargo test -p ffg-litellm` — 8 testes passando.
- `cargo test -p ffg-proxy` — 5 testes passando (CA, túnel e MITM com rewrite).
- `cargo test -p ffg-postgres` — 4 testes passando.
- `cargo llvm-cov -p ffg-core --fail-under-lines 80` — 92,23% de linhas.
- `cargo llvm-cov -p ffg-runtime --fail-under-lines 70` — 86,47% de linhas.
- `cargo llvm-cov -p ffg-supervisor --fail-under-lines 70` — 75,44% de linhas.
- `cargo llvm-cov -p ffg-litellm --fail-under-lines 70` — 87,39% de linhas.
- `cargo llvm-cov -p ffg-proxy --fail-under-lines 70` — 78,73% de linhas.
- `cargo llvm-cov -p ffg-postgres --fail-under-lines 70` — 100,00% de linhas.
- `node scripts/manifest.mjs ...` — manifesto gerado e JSON validado.
- `manifest-signer` + `manifest-verify` — assinatura e verificação testadas localmente.
- `pnpm build`
- `python3 scripts/check_docs.py` — 13 markdown, 4 JSON.
- `cargo clippy -p frank-figma-gateway --all-targets --target x86_64-pc-windows-gnu -- -D warnings`
  com stubs de `windres`/`gcc`/`ar` (valida tipos/lints; recursos, keyring e link ficam para o CI Windows).

> O build completo do app Tauri não roda neste ambiente Linux por falta de `dbus-1`/WebKitGTK.
> A validação do app fica pendente para o Windows, conforme combinado.

### Próximo passo

- Spike obrigatório do Copilot: confirmar endpoints de credential/device flow no LiteLLM `v1.108.1`.
- Executar a checklist completa de validação no Windows acima.
- F11 — E2E, caos, release assinado, updater e desinstalação.
- Completar importação offline e testes de UI (Vitest/RTL/axe).

### Como validar no Windows

```powershell
cd src
pnpm install --frozen-lockfile
pnpm build
cargo fmt --all -- --check
cargo clippy -p ffg-core -p ffg-runtime -p ffg-supervisor -p ffg-litellm -p ffg-proxy -p ffg-postgres --all-targets -- -D warnings
cargo test -p ffg-core -p ffg-runtime -p ffg-supervisor -p ffg-litellm -p ffg-proxy -p ffg-postgres
cargo llvm-cov -p ffg-core --fail-under-lines 80
cargo llvm-cov -p ffg-runtime --fail-under-lines 70
cargo llvm-cov -p ffg-supervisor --fail-under-lines 70
cargo llvm-cov -p ffg-litellm --fail-under-lines 70
cargo llvm-cov -p ffg-proxy --fail-under-lines 70
cargo llvm-cov -p ffg-postgres --fail-under-lines 70
cargo clippy -p frank-figma-gateway --all-targets -- -D warnings
pnpm tauri build --no-bundle
```

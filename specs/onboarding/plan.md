# Plano de implementação — onboarding completo (Windows x64)

> Status: plano de implementação — o código ainda não foi alterado. Base: `docs/onboarding.md`, `docs/desktop-app.md`, `docs/gateway-architecture.md`, `docs/figma-gateway-integration.md`, `docs/figma-llm-architecture.md`, `docs/gateway-prd.md`, `docs/dev-guide.md`, `AGENTS.md` e o scaffold em `src/`.

## 1. Resumo executivo

O onboarding completo só existe quando o app consegue, sem admin e sem alterar o Figma:

1. baixar, verificar e instalar o runtime LiteLLM `v1.108.1`;
2. subir LiteLLM (e Postgres, no perfil Copilot) de forma supervisionada;
3. configurar provedores por API key ou GitHub Copilot via OAuth;
4. gerar o `config.yaml`, aplicar aliases `claude-opus-4-8`/`claude-haiku-4-5` e reiniciar o LiteLLM com rollback;
5. gerar/confiar na CA local, subir o proxy MITM, gravar `HTTPS_PROXY`/`NO_PROXY` em `HKCU\Environment`, validar TLS + rewrite e reiniciar o Figma;
6. testar o fluxo ponta a ponta e permitir reconfiguração/remoção reversível.

O plano foi dividido em **12 fases (F0–F11)**. A entrega é única — nenhuma fase isolada libera o onboarding —, mas a ordem permite PRs pequenos e testes por camada. O caminho crítico é:

```text
F0 → F1 → F2 → F3 → F4 → F5 → F9 → F10 → F11
                  └→ F6 → F7 → F8 → F10
```

Estimativa realista: **27–40 pessoa-semanas** para um desenvolvedor sênior, sendo proxy MITM (F7), Copilot/Postgres (F9) e wizard (F10) os maiores blocos. Com dois desenvolvedores, o calendário cai, mas o total de esforço não.

Os maiores riscos que podem mudar o plano são: ausência de API estável do Copilot no LiteLLM `v1.108.1`, bloqueio corporativo da CA em `CurrentUser\Root`, complexidade de TLS/HTTP/2/SSE/Upgrade no proxy e empacotamento do runtime Python no Windows.

---

## 2. Premissas, escopo e não escopo

### 2.1 Premissas

- **Windows 10/11 x64 é o alvo primário.** Linux serve para `ffg-core`, `ffg-runtime` e `ffg-proxy`; macOS não é validado.
- O scaffold Tauri existente é reaproveitado: tray, autostart, single-instance, close-to-tray, log e notification.
- **Não reimplementar adapters de provider.** O LiteLLM é a camada de tradução; nosso código só faz integração, configuração, supervisão e proxy.
- **Não alterar binários, UI ou autenticação do Figma.** A integração é rede + trust store + env.
- **Sem admin.** CA em `CurrentUser\Root`, env em `HKCU\Environment`, processos por usuário.
- **Loopback apenas.** LiteLLM, Postgres e proxy escutam em `127.0.0.1`; portas dinâmicas persistidas.
- **Versões fixadas.** LiteLLM `v1.108.1`; nunca `v1.107.0`. Postgres 16.x, patch definido pelo manifesto.
- **Segredos fora de arquivos de estado.** Credential Manager/DPAPI; `setup.json` só guarda referências.
- **TDD.** Teste primeiro para comportamento novo; `cargo fmt`, clippy `-D warnings`, testes e cobertura antes de concluir.
- **Sem invenção de fatos.** Endpoints e nomes de modelo do Copilot marcados como “validar no spike” não devem ser tratados como verdade até confirmação na versão fixada.
- **Sem documentação não solicitada.** Este plano prevê apenas atualização dos docs afetados e ADRs das decisões estruturais exigidas pelo `AGENTS.md`.

### 2.2 Fora de escopo global

- Suporte a macOS/Linux no app desktop completo.
- i18n; MVP em pt-BR.
- Múltiplos perfis por projeto/time.
- Telemetria, crash reporting remoto ou analytics.
- Patch do Figma, do `figcode` ou do Claude Code.
- Contornar autenticação, licenciamento ou limites de plano.
- Uso do diretório `enterprise/` do LiteLLM.
- Auto-update silencioso de runtime; atualização é release nossa.
- Fail-open completo para requests streaming (avaliar no MVP como fail-closed; ver decisões em aberto).
- Autenticação forte no proxy local (ver risco de injeção de master key e decisão em aberto).

### 2.3 Definition of Done do onboarding

O onboarding é considerado concluído quando, em uma VM Windows limpa, sem admin e sem runtime instalado:

- o instalador tem 15–30 MB e abre o app;
- o wizard conclui os 5 passos do `docs/onboarding.md`;
- o perfil API key e o perfil Copilot funcionam;
- o modo offline “Já tenho os pacotes” instala via ZIP + manifesto assinado;
- fechar/reabrir o app retoma no passo pendente;
- “Reconfigurar” pelo tray reabre o wizard sem apagar credenciais;
- a integração Figma é instalada e removida com restauração exata do env;
- o tráfego não-IA do `api.figma.com` continua intacto;
- o teste ponta a ponta passa com provider real ou fake de CI;
- `cargo fmt`, clippy, testes, cobertura ≥80% no `ffg-core`, `pnpm build` e E2E Windows estão verdes.

---

## 3. Arquitetura alvo

### 3.1 Camadas e crates

```text
src/
├─ crates/
│  ├─ core/       # ffg-core: lógica pura, sem I/O, sem Tauri, sem Windows
│  ├─ runtime/    # ffg-runtime: download, resume, verificação, extração, instalação atômica
│  └─ proxy/      # ffg-proxy: CA, TLS, CONNECT, MITM seletivo, rewrite, streaming
├─ src-tauri/     # shell Tauri, supervisor, platform Windows, orquestração, comandos
└─ ui/            # React + Vite, wizard, dashboard, providers, logs, settings
```

| Camada | Responsabilidade | Testável em Linux? |
|---|---|---|
| `ffg-core` | Estado do setup, manifesto/assinatura, plano de download/instalação, config LiteLLM, catálogo de providers, env plan, journal, redaction, checks, modelos | Sim, sem rede |
| `ffg-runtime` | HTTP Range/pausa/retomada, hash, extração ZIP segura, instalação atômica, ponteiro ativo, probe `--version`, import offline | Sim, com fake HTTP server |
| `ffg-proxy` | Geração de CA/leaf, servidor proxy, CONNECT, MITM seletivo, túnel, rewrite, auth, SSE/streaming, estatísticas | Sim, com upstream TLS fake |
| `src-tauri` | Comandos IPC, estado do app, supervisor de processos, Job Object, Credential Manager, DPAPI, registro/env, detecção/restart do Figma, Postgres, LiteLLM client, orquestração do onboarding | Parcial: lógica fora do Windows; integração Windows no CI Windows |
| `ui` | Wizard, dashboard, providers/modelos, logs, settings; consome apenas comandos/eventos tipados | Sim, com bridge mockada |

**Decisão estrutural:** criar `ffg-runtime` e `ffg-proxy` como crates separados para não colocar I/O pesado e TLS dentro do `ffg-core` nem dentro do binário Tauri. Isso permite testes cross-platform e evita que o gate de cobertura do core seja contaminado por código de sistema.

### 3.2 Layout de dados

Usar `app.path().app_local_data_dir()` (no Windows, abaixo de `%LOCALAPPDATA%`; não hardcodar o identificador). Layout proposto:

```text
<app_local_data_dir>/
├─ state/
│  ├─ setup.json                 # estado do onboarding (sem segredos)
│  ├─ setup.json.bak             # cópia anterior
│  ├─ integration.json           # journal da integração Figma
│  └─ operations/                # metadados de operações longas/cancelamento
├─ runtime/
│  ├─ litellm/<versao>/          # instalação versionada
│  ├─ pgsql/<versao>/
│  ├─ staging/<uuid>/            # extração temporária
│  └─ active.json                # ponteiro ativo + versão anterior
├─ cache/downloads/
│  ├─ litellm-runtime-<versao>.zip.part
│  ├─ litellm-runtime-<versao>.zip.meta.json
│  └─ ...
├─ config/litellm/
│  ├─ config.yaml
│  ├─ history/<revision>.yaml
│  └─ candidates/<uuid>.yaml
├─ postgres/data/                # somente perfil Copilot
├─ ca/
│  ├─ root.pem
│  └─ root.key.dpapi             # chave privada protegida por DPAPI
├─ logs/
└─ diagnostics/                  # exports redigidos, sob demanda
```

### 3.3 Processos, startup e shutdown

```text
App Tauri
 ├─ supervisor ──> postgres.exe   (somente perfil Copilot)
 │                litellm.exe    (sempre, quando configurado)
 │                proxy interno  (thread/task Rust, quando integração ativa)
 ├─ proxy ───────> LiteLLM        (127.0.0.1:<porta>)
 └─ LiteLLM ─────> providers     (direto ou via proxy corporativo original, nunca via nosso proxy)
```

**Startup:**

1. single-instance check;
2. logging;
3. carrega `setup.json` e repara journal/estado parcial;
4. resolve segredos (master key/salt key; gera se necessário);
5. se o setup já tem runtime e configuração aplicada: sobe Postgres (se Copilot), LiteLLM, proxy;
6. garante a integração (CA/env) se estava ativa;
7. tray verde/amarelo/vermelho;
8. first-run ou erro → abre wizard/dashboard; autostart → permanece oculto.

**Shutdown:** proxy → LiteLLM → Postgres; timeout e kill tree. No Windows, Job Object com `KILL_ON_JOB_CLOSE` garante que filhos não fiquem órfãos. Ao sair, remover as variáveis de proxy do Figma (conforme `docs/desktop-app.md`), manter a CA instalada e avisar que o Figma precisa ser reiniciado. Na próxima abertura, reaplicar o env se a integração estiver ativa.

### 3.4 Invariantes de segurança

- Nenhum segredo em `setup.json`, `integration.json`, `config.yaml`, logs ou eventos da UI.
- `LITELLM_MASTER_KEY` aleatória por instalação; `LITELLM_SALT_KEY` estável (sem ela, tokens do Copilot ficam ilegíveis).
- API keys no Credential Manager (`keyring`); chave privada da CA protegida por DPAPI e ACL do usuário.
- Logs com `Authorization`, `x-api-key`, `cookie`, `proxy-authorization`, `set-cookie` e query/body sensíveis redigidos.
- Prompt/resposta desligados por padrão; diagnóstico só com opt-in.
- Bind exclusivamente `127.0.0.1`; sem inbound firewall; sem admin.
- LiteLLM e downloader **não** herdam o `HTTPS_PROXY` do nosso proxy; usar o proxy corporativo original, se existir, ou conexão direta. Evitar loop.
- Toda operação destrutiva tem journal e rollback.

---

## 4. Contratos transversais

### 4.1 `SetupState` e `setup.json`

Tipos puros em `src/crates/core/src/setup.rs`:

```rust
pub const SETUP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepId { Welcome, Runtime, Provider, Models, Figma, Finish }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus { Pending, InProgress, Done, Skipped, Failed }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepState {
    pub id: StepId,
    pub status: StepStatus,
    pub stale: bool,
    pub completed_at: Option<String>,
    pub skipped_reason: Option<String>,
    pub last_error: Option<UiError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    pub schema_version: u32,
    pub revision: u64,
    pub status: SetupStatus,          // notStarted | inProgress | completed | needsAttention
    pub current_step: StepId,
    pub steps: Vec<StepState>,
    pub runtime: RuntimeState,
    pub provider: Option<ProviderSelection>,
    pub copilot: CopilotState,
    pub models: ModelSettings,
    pub services: ServicePorts,
    pub figma: FigmaState,
    pub onboarding: OnboardingState,
    pub updated_at: String,
}
```

Subtipos principais:

```rust
pub struct RuntimeState {
    pub litellm_version: Option<String>,
    pub active_litellm: Option<String>,
    pub pgsql_version: Option<String>,
    pub active_pgsql: Option<String>,
}

pub struct ServicePorts {
    pub litellm: Option<u16>,
    pub proxy: Option<u16>,
    pub postgres: Option<u16>,
}

pub struct ProviderSelection {
    pub profile_id: String,
    pub kind: ProviderKind,          // openRouter | azureAi | anthropic | openAi | gemini | gitHubCopilot | openAiCompatible
    pub label: String,
    pub api_base: Option<String>,
    pub api_version: Option<String>,
    pub deployment: Option<String>,
    pub secret_ref: Option<String>,  // ex.: "ffg/provider/openrouter"; nunca o segredo
    pub test_status: TestStatus,     // notTested | ok | failed | skipped
    pub tested_at: Option<String>,
}

pub struct ModelSettings {
    pub routes: Vec<ModelRoute>,
    pub fallbacks: BTreeMap<String, Vec<String>>,
    pub applied_at: Option<String>,
    pub config_revision: Option<String>,
}

pub struct ModelRoute {
    pub public_name: String,         // claude-opus-4-8 / claude-haiku-4-5
    pub provider_model: String,      // ex.: github_copilot/<modelo-validado>
    pub credential_name: Option<String>,
    pub api_key_env: Option<String>,
    pub api_base: Option<String>,
    pub extra: BTreeMap<String, String>,
}

pub struct CopilotState {
    pub enabled: bool,
    pub postgres_ready: bool,
    pub database_name: String,       // "litellm"
    pub credential_name: String,     // "github-copilot"
    pub connected: bool,
    pub github_login: Option<String>,
    pub auth_status: AuthStatus,     // notStarted | waitingForUser | connected | expired | failed
    pub last_error: Option<UiError>,
}

pub struct FigmaState {
    pub detected: bool,
    pub install_path: Option<String>,
    pub version: Option<String>,
    pub running: bool,
    pub integration_active: bool,
    pub ca_thumbprint: Option<String>,
    pub env_installed: bool,
    pub proxy_port: Option<u16>,
    pub last_validated_at: Option<String>,
    pub skipped: bool,
}

pub struct OnboardingState {
    pub offline_mode: bool,
    pub skipped_figma: bool,
    pub start_with_windows: bool,
    pub completed_at: Option<String>,
}
```

Exemplo de `setup.json` (sem segredos):

```json
{
  "schemaVersion": 1,
  "revision": 12,
  "status": "inProgress",
  "currentStep": "figma",
  "steps": [
    { "id": "welcome", "status": "done", "stale": false, "completedAt": "2026-11-06T10:00:00Z" },
    { "id": "runtime", "status": "done", "stale": false, "completedAt": "2026-11-06T10:04:00Z" },
    { "id": "provider", "status": "done", "stale": false, "completedAt": "2026-11-06T10:07:00Z" },
    { "id": "models", "status": "done", "stale": false, "completedAt": "2026-11-06T10:08:00Z" },
    { "id": "figma", "status": "inProgress", "stale": false },
    { "id": "finish", "status": "pending", "stale": false }
  ],
  "runtime": {
    "litellmVersion": "1.108.1",
    "activeLitellm": "1.108.1",
    "pgsqlVersion": "16.9",
    "activePgsql": "16.9"
  },
  "provider": {
    "profileId": "copilot",
    "kind": "gitHubCopilot",
    "label": "GitHub Copilot",
    "secretRef": null,
    "testStatus": "ok",
    "testedAt": "2026-11-06T10:07:00Z"
  },
  "copilot": {
    "enabled": true,
    "postgresReady": true,
    "databaseName": "litellm",
    "credentialName": "github-copilot",
    "connected": true,
    "githubLogin": "octocat",
    "authStatus": "connected"
  },
  "models": {
    "routes": [
      {
        "publicName": "claude-opus-4-8",
        "providerModel": "github_copilot/<modelo-validado>",
        "credentialName": "github-copilot"
      },
      {
        "publicName": "claude-haiku-4-5",
        "providerModel": "github_copilot/<modelo-validado>",
        "credentialName": "github-copilot"
      }
    ],
    "fallbacks": { "claude-opus-4-8": ["claude-haiku-4-5"] },
    "appliedAt": "2026-11-06T10:08:00Z",
    "configRevision": "sha256:..."
  },
  "services": { "litellm": 4000, "proxy": 8787, "postgres": 54329 },
  "figma": {
    "detected": true,
    "installPath": "C:\\Users\\usuario\\AppData\\Local\\Figma\\Figma.exe",
    "version": "126.11.7",
    "running": false,
    "integrationActive": false,
    "caThumbprint": null,
    "envInstalled": false,
    "proxyPort": 8787,
    "lastValidatedAt": null,
    "skipped": false
  },
  "onboarding": {
    "offlineMode": false,
    "skippedFigma": false,
    "startWithWindows": false,
    "completedAt": null
  },
  "updatedAt": "2026-11-06T10:08:00Z"
}
```

Regras de persistência:

- escrita atômica: temp no mesmo diretório → `fsync` → `ReplaceFileW`/rename → `.bak`;
- `schemaVersion` maior que o app: recusar alteração e pedir atualização;
- `schemaVersion` menor: migração pura em `ffg-core::setup` com testes de fixture;
- JSON corrompido: tentar `.bak`; se falhar, quarentena `<arquivo>.corrupt-<ts>` e iniciar estado novo com `needsAttention`;
- `revision` incrementa a cada mutação; a UI ignora eventos com revisão menor;
- ACL do arquivo restrita ao usuário; nada de segredo no arquivo;
- paths e `githubLogin` são dados locais; não vão para logs/eventos de erro.

### 4.2 Manifesto, assinatura e política de versão

Formato proposto para `manifest.json` (payload assinado byte a byte):

```json
{
  "schemaVersion": 1,
  "generatedAt": "2026-11-06T12:00:00Z",
  "minAppVersion": "0.1.0",
  "litellm": {
    "version": "1.108.1",
    "url": "https://github.com/<org>/<repo>/releases/download/v0.1.0/litellm-runtime-1.108.1-win-x64.zip",
    "sha256": "<64 hex>",
    "size": 123456789,
    "installedSize": 345678901,
    "entrypoint": "litellm.exe",
    "versionArgs": ["--version"]
  },
  "pgsql": {
    "version": "16.9",
    "url": "https://github.com/<org>/<repo>/releases/download/v0.1.0/pgsql-16.9-win-x64.zip",
    "sha256": "<64 hex>",
    "size": 45678901,
    "installedSize": 123456789,
    "entrypoint": "bin/postgres.exe",
    "versionArgs": ["--version"]
  }
}
```

Assinatura destacada `manifest.json.sig`:

```json
{ "keyId": "2026-01", "algorithm": "ed25519", "signature": "<base64>" }
```

Decisões:

- A assinatura cobre **os bytes exatos** do `manifest.json` (evita canonicalização JSON divergente).
- A chave pública fica embutida em `src-tauri/resources/keys/manifest-keys.json`, com `keyId`, `notBefore`, `notAfter` e `status`.
- O app aceita apenas `keyId` conhecido e ativo; chave desconhecida → erro “atualize o app”.
- Rotação: o release inclui a chave atual + próxima; o app precisa de atualização para confiar em uma chave nova. Não há revogação remota no MVP.
- `minAppVersion` é comparado por semver. Componente fora da política (`RuntimePolicy`) é rejeitado: LiteLLM deve ser exatamente `1.108.1`; Postgres deve ser major 16.
- A verificação ocorre antes de qualquer download e novamente antes de extrair. Hash inválido **nunca** extrai.
- Cache do último manifesto válido para modo offline, com aviso de idade; a assinatura é reverificada ao usar o cache.

### 4.3 Comandos Tauri

Todos os comandos longos são `async`, recebem `operation_id` quando canceláveis e retornam `Result<T, UiError>`. Nomes de comando em `snake_case`; payloads em `camelCase`.

| Domínio | Comando | Fase | Entrada principal | Saída |
|---|---|---|---|---|
| App | `app_info` | existente | — | `AppInfo` |
| App | `diagnostics_environment` | F0/F11 | — | `EnvironmentReport` |
| App | `diagnostics_export` | F11 | `path` | `ExportResult` |
| Setup | `setup_get_state` | F0 | — | `SetupState` |
| Setup | `setup_begin_step` | F0 | `step` | `SetupState` |
| Setup | `setup_complete_step` | F0 | `step`, `outcome` | `SetupState` |
| Setup | `setup_set_options` | F0/F10 | `offlineMode?`, `startWithWindows?` | `SetupState` |
| Setup | `setup_reset` | F0/F10 | `scope` | `SetupState` |
| Runtime | `runtime_status` | F3 | — | `RuntimeStatus` |
| Runtime | `runtime_download` | F3 | `component`, `options`, `operationId` | `DownloadStarted` |
| Runtime | `runtime_pause` | F3 | `downloadId` | `DownloadState` |
| Runtime | `runtime_resume` | F3 | `downloadId` | `DownloadState` |
| Runtime | `runtime_cancel` | F3 | `downloadId`, `removePartial` | `DownloadState` |
| Runtime | `runtime_import_offline` | F3 | `component`, `zipPath`, `manifestPath?`, `sigPath?`, `operationId` | `InstalledComponent` |
| Runtime | `runtime_activate` | F3 | `component`, `version` | `RuntimeStatus` |
| Serviços | `service_status` | existente/F4 | — | `ServiceStatus[]` |
| Serviços | `start_services` | existente/F4 | — | `ServiceStatus[]` |
| Serviços | `stop_services` | existente/F4 | — | `ServiceStatus[]` |
| Serviços | `restart_services` | F4 | `services?` | `ServiceStatus[]` |
| Serviços | `service_logs` | F4 | `service`, `cursor?`, `limit?` | `LogPage` |
| Providers | `providers_catalog` | F5 | — | `ProviderDescriptor[]` |
| Providers | `provider_test` | F5 | `profile`, `operationId` | `ProviderTestResult` |
| Providers | `provider_save` | F5 | `profile`, `operationId` | `SetupState` |
| Providers | `provider_remove` | F5 | `profileId` | `SetupState` |
| Providers | `provider_secret_status` | F5 | `profileId` | `SecretStatus` |
| Copilot | `copilot_status` | F9 | — | `CopilotStatus` |
| Copilot | `copilot_prepare` | F9 | `operationId` | `CopilotStatus` |
| Copilot | `copilot_begin_auth` | F9 | — | `DeviceFlow` |
| Copilot | `copilot_poll_auth` | F9 | — | `CopilotStatus` |
| Copilot | `copilot_disconnect` | F9 | — | `CopilotStatus` |
| Copilot | `copilot_disable` | F9 | `deleteData?` | `SetupState` |
| Modelos | `models_get` | F5/F10 | — | `ModelSettings` |
| Modelos | `models_test` | F5 | `settings` | `ModelTestReport` |
| Modelos | `models_apply` | F5 | `settings`, `operationId` | `SetupState` |
| Figma | `figma_environment` | F8 | — | `FigmaEnvironment` |
| Figma | `figma_install` | F8 | `options`, `operationId` | `IntegrationStatus` |
| Figma | `figma_validate` | F8 | — | `IntegrationValidation` |
| Figma | `figma_remove` | F8 | `operationId` | `IntegrationStatus` |
| Figma | `figma_restart` | F8 | — | `FigmaRestartResult` |
| Figma | `integration_status` | F8 | — | `IntegrationStatus` |
| Proxy | `proxy_status` | F7 | — | `ProxyStatus` |
| Proxy | `proxy_self_test` | F7/F8 | — | `ProxySelfTest` |
| CA | `ca_status` | F6 | — | `CaStatus` |
| CA | `ca_install` | F6 | — | `CaStatus` |
| CA | `ca_remove` | F6 | — | `CaStatus` |
| CA | `ca_export` | F6/F8 | `path` | `ExportResult` |
| Operações | `operation_cancel` | F0 | `operationId` | `()` |
| Janelas | `open_litellm_ui` | F7/F10 | — | `()` |
| Sistema | `open_logs_folder` / `open_data_folder` | F10 | — | `()` |
| Autostart | `autostart_get` / `autostart_set` | F10 | `enabled` | `bool` |

Compatibilidade: manter `install_figma_integration` e `remove_figma_integration` como wrappers dos novos `figma_install`/`figma_remove` até a UI migrar; `service_status` passa a devolver o enum estendido (`running`, `degraded`, `failed`, `stopping`).

### 4.4 Eventos para a UI

| Evento | Quando | Payload |
|---|---|---|
| `setup://state` | toda mutação de `SetupState` | `SetupState` completo |
| `download://progress` | progresso de download/verificação/extração | `DownloadProgress` |
| `download://state` | mudança de estado do download | `DownloadStateEvent` |
| `service://status` | mudança de estado/saúde dos sidecars | `ServiceStatus` |
| `copilot://auth` | device flow/poll | `CopilotAuthEvent` |
| `integration://status` | instalação/validação/rollback do Figma | `IntegrationStatus` |
| `proxy://first-ai-request` | primeira requisição de IA real atravessa o proxy | `FirstAiRequest` |
| `wizard://open` | tray/pedido de reabertura | `WizardOpenRequest` |
| `app://error` | erro background sem comando pendente | `UiError` |
| `operation://progress` | operações longas que não são download | `OperationProgress` |

`DownloadProgress`:

```ts
interface DownloadProgress {
  downloadId: string;
  component: "litellm" | "pgsql";
  status:
    | "queued" | "downloading" | "verifying" | "extracting"
    | "probing" | "installing" | "done" | "paused" | "cancelled" | "failed";
  bytesDownloaded: number;
  totalBytes?: number;
  bytesPerSecond?: number;
  etaSeconds?: number;
  attempt: number;
  error?: UiError;
}
```

`ServiceStatus`:

```ts
interface ServiceStatus {
  name: "postgres" | "litellm" | "proxy";
  state: "stopped" | "starting" | "running" | "degraded" | "failed" | "stopping";
  detail?: string | null;
  port?: number | null;
  health?: "unknown" | "ok" | "timeout" | "error";
  restartCount: number;
  lastError?: UiError | null;
}
```

`FirstAiRequest` não carrega prompt nem corpo:

```ts
interface FirstAiRequest {
  at: string;
  host: string;
  path: string;      // path template, sem query sensível
  status: number;
  model?: string;
}
```

### 4.5 Tipos TypeScript e bridge

Arquivos:

- `src/ui/src/lib/contracts.ts` — tipos espelhando Rust, unions de estado, `UiError`;
- `src/ui/src/lib/events.ts` — wrappers `listen` tipados e cleanup;
- `src/ui/src/lib/bridge.ts` — todas as chamadas `invoke`, nenhuma string solta em componente;
- `src/ui/src/state/AppStore.tsx` — contexto/`useReducer`, carrega `setup`, assina eventos, mantém `screen`, operações em andamento, toasts;
- componentes nunca chamam `invoke` diretamente.

Exemplo de bridge:

```ts
export async function getSetupState(): Promise<SetupState> {
  return invoke<SetupState>("setup_get_state");
}

export async function completeStep(
  step: StepId,
  outcome: StepOutcome,
): Promise<SetupState> {
  return invoke<SetupState>("setup_complete_step", { step, outcome });
}

export function onSetupState(
  cb: (state: SetupState) => void,
): Promise<UnlistenFn> {
  return listen<SetupState>("setup://state", (event) => cb(event.payload));
}
```

Regras de tipagem: `strict` já ligado; sem `any`; erros sempre `UiError`; `StepId` e `ServiceState` como unions literais; nenhum segredo em tipos de retorno.

### 4.6 Configuração gerada do LiteLLM

`ffg-core::litellm_config` gera YAML determinístico. Forma:

```yaml
model_list:
  - model_name: claude-opus-4-8
    litellm_params:
      model: <provider_model>
      # API key providers:
      api_key: os.environ/OPENROUTER_API_KEY
      # Copilot:
      # litellm_credential_name: github-copilot
      api_base: <opcional>
      api_version: <opcional>

  - model_name: claude-haiku-4-5
    litellm_params:
      model: <provider_model>
      api_key: os.environ/OPENROUTER_API_KEY

router_settings:
  fallbacks:
    - claude-opus-4-8: ["claude-haiku-4-5"]
  num_retries: 2

general_settings:
  master_key: os.environ/LITELLM_MASTER_KEY
```

Regras:

- Nunca escrever API key literal no YAML; sempre `os.environ/<VAR>`.
- `DATABASE_URL` só entra no ambiente do processo quando o perfil Copilot está ativo; não escrever a senha no YAML.
- `litellm_credential_name` só para Copilot.
- Validar o YAML gerado (parse) antes de gravar.
- Guardar histórico em `config/litellm/history/<revision>.yaml`; candidato em `candidates/<uuid>.yaml`.
- `store_model_in_db` e a compatibilidade exata de `litellm_credential_name` com `/model/new` devem ser confirmadas no spike da versão `v1.108.1`.

### 4.7 Journal da integração

`state/integration.json` registra cada efeito colateral para rollback:

```json
{
  "schemaVersion": 1,
  "status": "active",
  "ca": {
    "thumbprint": "<sha1>",
    "installedAt": "2026-11-06T10:20:00Z"
  },
  "env": {
    "snapshotRef": "ffg/integration/env-snapshot",
    "installed": {
      "HTTPS_PROXY": "http://127.0.0.1:8787",
      "HTTP_PROXY": "http://127.0.0.1:8787",
      "NO_PROXY": "localhost,127.0.0.1,::1"
    }
  },
  "proxy": { "port": 8787 },
  "figma": { "version": "126.11.7", "restartedAt": "2026-11-06T10:21:00Z" },
  "steps": [
    { "id": "ca_install", "status": "done", "at": "2026-11-06T10:19:00Z" },
    { "id": "proxy_start", "status": "done", "at": "2026-11-06T10:19:30Z" },
    { "id": "env_write", "status": "done", "at": "2026-11-06T10:20:00Z" },
    { "id": "validate", "status": "done", "at": "2026-11-06T10:21:00Z" }
  ]
}
```

- Snapshot do env fica no Credential Manager/DPAPI, porque pode conter proxy corporativo com usuário/senha.
- Rollback percorre `steps` em ordem inversa: `env_restore` → `proxy_stop` → `ca_remove`.
- `status: partial` na inicialização dispara reparo/rollback e banner no dashboard.
- Remoção idempotente: repetir `figma_remove` não falha.

### 4.8 Erros

```rust
pub struct UiError {
    pub code: ErrorCode,
    pub message: String,        // pt-BR, acionável
    pub detail: Option<String>, // sanitizado, para "Copiar diagnóstico"
    pub retryable: bool,
    pub action: Option<String>, // ex.: "retry_download" | "reopen_figma"
}

pub enum ErrorCode {
    UnsupportedPlatform, DataDirNotWritable, DiskSpace, Network, Timeout,
    ManifestSignature, ManifestSchema, AppTooOld, ComponentNotAllowed,
    HashMismatch, SizeMismatch, RangeInvalid, ZipUnsafe, Extraction,
    VersionProbe, VersionMismatch, Activation, Rollback,
    PortInUse, ProcessSpawn, ProcessExited, HealthTimeout,
    ProviderAuth, ProviderForbidden, ProviderModelNotFound, ProviderRateLimited,
    ProviderBadRequest, ProviderUnavailable,
    CopilotAuthPending, CopilotAuthExpired, CopilotPlanMissing, CopilotDatabase,
    CaGeneration, CaTrust, CaRemove, EnvRead, EnvWrite, EnvRestore,
    FigmaNotFound, FigmaRunning, FigmaRestart, ProxyTls, ProxyRewrite, ProxyUpstream,
    ConfigInvalid, ConfigApply, Internal,
}
```

Nenhum `UiError` deve conter chave, token, URL com userinfo, prompt ou corpo de resposta. `#[serde(rename_all = "camelCase")]` no struct; `code` como string estável.

### 4.9 Operações longas e cancelamento

- `OperationId` é gerado pela UI (`crypto.randomUUID()`) e passado ao comando.
- O backend registra um `CancellationToken` (`tokio-util`) por operação.
- `operation_cancel` cancela o token; o comando retorna `UiError { code: Timeout/Internal, ... }` ou um resultado parcial explícito.
- Downloads têm `DownloadId` além do `OperationId`, porque sobrevivem ao fechamento do app via `.part` + `.meta.json`.
- Toda operação longa emite progresso, mas a UI faz throttle de renderização (100–250 ms).

---

## 5. Desenho do wizard de onboarding

> **Formato decidido:** modal overlay sobre o dashboard, com carrossel horizontal de uma etapa por vez.
> Fechar (`Esc`, botão ou backdrop) retorna ao dashboard sem perder o estado; downloads/OAuth longos
> continuam em background e podem ser acompanhados pelo tray. Navegar no carrossel não altera o
> `setup.json`; só a conclusão real de cada etapa o faz.

### 5.1 Passos

| Passo | Tela | Conteúdo | Conclusão | Pode pular? |
|---|---|---|---|---|
| 0 | Boas-vindas | Checks de Windows x64, disco, internet, Figma; 3 bullets do que será instalado; “Continuar” e “Já tenho os pacotes” | Checks bloqueantes OK | Não |
| 1 | Baixar LiteLLM | Download com progresso, pausa/retomada, velocidade, cancelar, espelhos, importar ZIP; badge de assinatura/hash/versão | LiteLLM ativo e `--version` validado | Não |
| 2 | Escolher provedor | Cards OpenRouter, Azure AI, Anthropic, OpenAI, Gemini, GitHub Copilot; formulário; “Testar conexão” | Perfil salvo + teste OK, ou “pular com aviso” confirmado | Sim, com warning |
| 2b | Copilot | Sub-fluxo: baixar Postgres, inicializar, subir, criar credential, OAuth/device flow, testar | “Connected” + teste OK, ou pular | Sim, com warning |
| 3 | Modelos e fallback | `claude-opus-4-8` e `claude-haiku-4-5`; modelo do provedor; fallback; preview do YAML; “Aplicar e testar” | Config aplicada + LiteLLM saudável + teste OK | Não |
| 4 | Conectar Figma | Detecção, versão, o que muda, instalar integração, checklist CA/env/proxy/TLS/rewrite, reiniciar Figma | Integração ativa + validação OK, ou pular | Sim, com badge pendente |
| 5 | Teste/conclusão | Grade de status (Postgres, LiteLLM, proxy, CA, env, Figma), teste ponta a ponta opcional, autostart, “Abrir dashboard”, “Concluir” | Status OK ou warnings aceitos | Não |

O sub-fluxo Copilot não é um passo numerado separado; ele aparece dentro do passo 2, com progresso próprio, para não quebrar a jornada de 5 passos do `docs/onboarding.md`.

### 5.2 Máquina de estados

`ffg-core::setup` implementa as transições puras:

```rust
pub fn can_begin(state: &SetupState, step: StepId) -> Result<(), SetupError>;
pub fn can_complete(state: &SetupState, step: StepId) -> Result<(), SetupError>;
pub fn complete(state: &mut SetupState, step: StepId, outcome: StepOutcome);
pub fn first_pending_step(state: &SetupState) -> StepId;
pub fn invalidate_downstream(state: &mut SetupState, from: StepId);
pub fn reset_scope(state: &mut SetupState, scope: ResetScope) -> SetupState;
```

Regras:

- `Done` só é aceito se a condição de conclusão do passo for verdadeira.
- `Skipped` exige `reason` e fica visível no dashboard/finish.
- `stale: true` em passos a jusante quando uma escolha anterior muda:
  - provedor alterado → modelos `stale`, teste do Figma `stale`;
  - modelos alterados → finish `stale`;
  - runtime alterado → serviços reiniciados; todos os testes `stale`;
  - integração removida → Figma `pending`.
- Voltar é permitido; não apaga artefatos. Só invalida o que depende da escolha.
- `SetupStatus::NeedsAttention` quando há `failed` ou integração `partial`.
- `first_pending_step` retorna o primeiro `pending`, `failed` ou `stale`, na ordem canônica.

### 5.3 Persistência e retomada

- Ao abrir: `setup_get_state`; se `status != completed`, abrir o modal no `first_pending_step`.
- Se `currentStep` tem `last_error`, abrir nele com banner e ação de retry.
- Fechar o modal não cancela o wizard: salva estado e mantém serviços já configurados; downloads/OAuth
  continuam em background.
- Reabrir pelo dashboard/tray restaura o carrossel no `currentStep`; a navegação entre etapas é apenas
  visual e não grava progresso.
- Fechar durante download: `.part` + `.meta.json` persistem; ao reabrir, oferecer “Retomar”.
- Fechar durante instalação: staging é limpo na inicialização; ponteiro ativo anterior permanece.
- Estado corrompido: `.bak`; se ambos falharem, quarentena e wizard do zero com aviso.
- `setup.json` é a única fonte de verdade do wizard; a UI não tem estado de passo próprio persistido.

### 5.4 Reconfigurar, reset e rollback

Menu do tray:

- **Abrir dashboard**
- **Reconfigurar** → evento `wizard://open` com `mode: "reconfigure"`, começando no passo 2 (Provider)
- **Configurar Figma** → `mode: "figmaOnly"`, passo 4
- **Abrir LiteLLM** → WebviewWindow externa (não iframe)
- **Reiniciar serviços**
- **Logs**
- **Sair**

Ações destrutivas:

| Ação | Efeito |
|---|---|
| “Refazer configuração” | Reset do estado do wizard; mantém runtime baixado e credenciais, salvo confirmação; para serviços |
| “Remover integração Figma” | Restaura env, para proxy, remove CA nossa; mantém LiteLLM/Postgres |
| “Apagar dados do Copilot” | Para Postgres, remove data dir e credencial; exige confirmação |
| “Desinstalar” | Remove integração, para serviços, apaga estado/segredos e, com confirmação, runtime/dados |

Rollback por etapa:

- instalação de runtime falha → ponteiro ativo anterior intacto;
- provider falha → config anterior restaurada e LiteLLM reiniciado;
- Copilot falha → Postgres pode ficar parado; estado `partial`; retry/remover;
- Figma falha → journal desfaz env/CA/proxy na ordem inversa;
- crash durante instalação → journal `partial` é reparado no próximo start.

### 5.5 Modo offline (“Já tenho os pacotes”)

- No passo 0, o botão alterna `offlineMode`.
- Passo 1 mostra “Importar pacote local” em vez de download.
- Entrada esperada: `manifest.json` + `manifest.json.sig` + ZIP(s). A assinatura é obrigatória por padrão.
- Se o usuário trouxer só o ZIP, o app calcula e mostra o SHA-256, mas **não instala** sem confirmação explícita em modo avançado; o estado fica marcado como `unverifiedLocal` e o finish mostra warning.
- Passo 2: “Testar conexão” fica indisponível; “Pular com aviso” exige confirmação.
- Copilot fica desabilitado (OAuth exige internet).
- Passo 4 pode instalar a integração offline (CA/env/proxy são locais), mas a validação de rewrite contra LiteLLM depende do runtime local.
- Ao detectar conectividade depois, oferecer “Testar agora” e limpar warnings.

### 5.6 Validações de ambiente

`EnvironmentReport`:

```rust
pub struct EnvironmentReport {
    pub os: String,
    pub arch: String,
    pub supported: bool,
    pub data_dir: String,
    pub free_disk_bytes: u64,
    pub required_disk_bytes: u64,
    pub checks: Vec<EnvironmentCheck>,
    pub figma: FigmaDetection,
    pub existing_proxy: ProxyEnvironment,
    pub ports: Vec<PortStatus>,
    pub blocking: bool,
    pub warnings: Vec<String>,
}

pub struct EnvironmentCheck {
    pub id: CheckId,        // platform | dataDir | disk | internet | figma | webview2 | ports | existingProxy | admin | clock
    pub status: CheckStatus, // pass | warn | fail | unknown
    pub message: String,
    pub detail: Option<String>,
}
```

Regras:

- **Bloqueante:** não-Windows x64; data dir não gravável; disco insuficiente; WebView2 ausente (o app já não abriria); porta sem alternativa.
- **Warning:** internet ausente em modo offline; Figma não encontrado; proxy preexistente; app rodando como admin (não recomendado); relógio muito dessincronizado.
- **Disco:** exigir `size + installedSize + 20%`; recalcular quando o componente escolhido muda (Postgres no Copilot).
- **Internet:** probe no host do manifesto, com timeout curto; nada de telemetria.
- **Figma:** detectar instalação/versão/processo; versão desconhecida gera warning, não bloqueio.
- **Proxy preexistente:** ler `HKCU\Environment` e WinINET; nunca logar userinfo; exigir escolha “substituir com snapshot” ou “manter e avisar”.
- **Admin:** detectar token elevado; não exigir; avisar que a integração é por usuário.
- **Portas:** tentar a persistida; se ocupada, alocar outra e persistir; se integração ativa, atualizar env e avisar para reiniciar o Figma.

### 5.7 Acessibilidade

- Navegação de passos com `<nav aria-label="Etapas da configuração">`, `aria-current="step"` e status textual (não só cor).
- Ao trocar de passo, mover foco para o `h1`/`h2` com `tabIndex={-1}` e anunciar via `aria-live="polite"`.
- Progresso com `role="progressbar"`, `aria-valuemin/max/now` e texto “X de Y MB”; throttle de anúncios para não inundar o leitor de tela.
- Erros com `role="alert"`, resumo no topo, foco no primeiro erro, `aria-describedby` nos campos.
- Formulários com `<label>`, `<fieldset>/<legend>`, `autocomplete` adequado; API key em `type="password"`.
- Botões desabilitados com motivo visível; não usar apenas cor para estado.
- Contraste ≥ 4.5:1; foco visível; alvos ≥ 44×44 px; `prefers-reduced-motion`.
- Diálogos com foco preso, `Esc` fecha, retorno de foco ao elemento de origem.
- Modal do wizard: `role="dialog"`, `aria-modal="true"`, backdrop clicável, foco preso e restaurado.
- Carrossel: navegação por botões e teclado; não depender de swipe; `prefers-reduced-motion` desativa
  a transição horizontal; cada etapa tem título focável e status anunciado via `aria-live`.
- Testes com `vitest-axe`/`axe-core` e navegação por teclado.

### 5.8 Estrutura de UI

```text
src/ui/src/
├─ App.tsx
├─ main.tsx
├─ styles.css
├─ lib/
│  ├─ bridge.ts
│  ├─ contracts.ts
│  ├─ events.ts
│  └─ format.ts
├─ state/
│  └─ AppStore.tsx
├─ screens/
│  ├─ Dashboard.tsx
│  ├─ wizard/
│  │  ├─ WizardModal.tsx      # overlay + carrossel + foco preso
│  │  ├─ wizard.css
│  │  ├─ StepWelcome.tsx
│  │  ├─ StepRuntime.tsx
│  │  ├─ StepProvider.tsx
│  │  ├─ StepModels.tsx
│  │  ├─ StepFigma.tsx
│  │  └─ StepFinish.tsx
│  ├─ providers/ProvidersScreen.tsx
│  ├─ logs/LogsScreen.tsx
│  └─ settings/SettingsScreen.tsx
├─ components/
│  ├─ ProgressSteps.tsx
│  ├─ StatusCard.tsx
│  ├─ Alert.tsx
│  ├─ ErrorBanner.tsx
│  ├─ ConfirmDialog.tsx
│  ├─ Field.tsx
│  └─ CodeBlock.tsx
└─ test/
   ├─ setup.ts
   └─ ...
```

A janela principal continua única; o wizard é um **modal overlay com carrossel horizontal**, não uma
janela separada no MVP. A UI do LiteLLM é uma `WebviewWindow` separada com `WebviewUrl::External` (não
iframe, por causa de `X-Frame-Options`).

---

## 6. Fluxos críticos

### 6.1 Download, verificação, instalação e rollback

1. **Manifesto:** buscar `manifest.json` + `manifest.json.sig` por HTTPS; se offline, usar cache/local. Verificar assinatura Ed25519 sobre os bytes exatos, `keyId` conhecido/ativo, `schemaVersion`, `minAppVersion` e `RuntimePolicy`.
2. **Preflight:** disco (`size + installedSize + 20%`), permissão no data dir, porta não aplicável, componente permitido.
3. **Escolha de espelho:** GitHub Releases primário; espelhos configuráveis; medir falha e trocar.
4. **HEAD/GET:** comparar `size` do manifesto; se `.part` + `.meta.json` compatíveis, enviar `Range: bytes=N-` e `If-Range` com ETag/Last-Modified.
5. **Validação de Range:** aceitar apenas `206` com `Content-Range` coerente; `200` em retomada → recomeçar; `416` → recomeçar; `Content-Length` divergente → erro.
6. **Streaming para `.part`:** hash SHA-256 incremental; progresso; pausa/cancelamento; retry com backoff e troca de espelho; nunca carregar o ZIP inteiro em memória.
7. **Conclusão:** conferir tamanho e SHA-256; renomear `.part` para `.zip`; só então extrair.
8. **Extração segura:** `staging/<uuid>`; rejeitar path absoluto, `..`, symlink, entradas demais, tamanho descomprimido acima do limite, ZIP criptografado; validar layout esperado (`entrypoint`).
9. **Probe:** executar `entrypoint --version` com timeout, janela oculta, env mínimo; comparar versão exata do manifesto. Postgres usa `postgres --version`/`pg_ctl --version`.
10. **Ativação atômica:** renomear staging para `runtime/<component>/<version>`; gravar `active.json` via temp + `ReplaceFileW`; manter a versão anterior; limpar staging.
11. **Rollback:** se qualquer etapa falhar, remover staging, manter ponteiro anterior e reportar `UiError` com ação.
12. **Offline:** mesmo pipeline com `ManifestSource::Local`; assinatura obrigatória por padrão.

Matriz de falhas:

| Falha | Comportamento |
|---|---|
| Rede/timeout | Retry 3× com backoff + jitter; troca de espelho; mantém `.part` |
| HTTP 404/403 | Troca de espelho; se todos falharem, erro com link |
| 200 em retomada | Recomeça do zero |
| 416 | Recomeça do zero |
| Tamanho divergente | Apaga `.part` e recomeça; se repetir, erro |
| SHA-256 inválido | Apaga ZIP/.part; **nunca extrai**; erro |
| Assinatura inválida | Aborta antes do download/extração; erro |
| Disco cheio | Para; mensagem com espaço necessário; limpeza opcional |
| ZIP inseguro | Remove staging; erro; não ativa |
| `--version` falha | Remove staging; mantém versão anterior |
| Falha ao ativar | Restaura ponteiro anterior |
| Cancelar | Para o stream; mantém `.part` se o usuário não pedir remoção |
| Fechar o app | Persiste `.meta.json`; retoma na próxima abertura |

### 6.2 Escolha de provedor, teste de conexão e aplicação

1. UI carrega `providers_catalog` (metadados, campos, se exige Postgres, env var esperada; sem segredos).
2. Usuário preenche; `provider_test` valida campos puros no core (URL, campos obrigatórios, modelo público `claude-*`/`anth-*`).
3. O segredo vai para a memória e, se o teste passar, para o Credential Manager; nunca para `setup.json`.
4. **Teste sem reimplementar adapter:** subir uma instância transitória do LiteLLM com o candidato de config em porta efêmera; esperar `/health/readiness`; chamar `POST /v1/messages` com `model` público e `max_tokens: 8`; parar a instância. Para Copilot, testar na instância principal/transitória com o mesmo DB, sem duas migrações concorrentes.
5. `provider_save` aplica de forma transacional:
   - grava `candidates/<uuid>.yaml`;
   - para o LiteLLM atual (se houver);
   - inicia com o candidato;
   - aguarda readiness;
   - testa;
   - em sucesso, promove para `config.yaml` + histórico e persiste o perfil;
   - em falha, restaura `config.yaml` anterior e reinicia o LiteLLM; devolve erro claro.
6. Mapeamento de erros: 401 → chave inválida; 403 → permissão/plano; 404 → modelo inexistente; 429 → rate limit; timeout/DNS/TLS → rede; 400 → payload/config.
7. “Pular com aviso” só no onboarding e com confirmação; grava `testStatus: skipped` e banner no dashboard.

Provedores por API key (OpenRouter, Azure AI, Anthropic, OpenAI, Gemini, OpenAI-compatible/local) usam o mesmo fluxo; Azure acrescenta `endpoint`, `api_version` e `deployment`; local aceita `api_base` e modelo. Os IDs de modelo recomendados ficam em um recurso versionado e são validados pelo teste; não hardcodar nomes que possam mudar sem confirmação.

### 6.3 GitHub Copilot + Postgres

1. **Pré-requisito:** perfil Copilot selecionado; `copilot_prepare` habilitado.
2. **Download do Postgres:** mesmo pipeline F3 (`pgsql-16.x-win-x64.zip`), só no perfil Copilot.
3. **Init do cluster:** `initdb -D <data> -U ffg --pwfile=<temp> --auth=scram-sha-256 --encoding=UTF8 --locale=C`; senha gerada e guardada no Credential Manager/DPAPI; arquivo temporário com ACL e remoção imediata. Testar caminhos com espaço/acentos.
4. **Start:** `postgres.exe -D <data> -p <port> -c listen_addresses=127.0.0.1` sob o supervisor/Job Object; aguardar `pg_isready`/TCP; criar database `litellm` via `psql` com `PGPASSWORD` por ambiente e SQL por stdin (sem shell).
5. **LiteLLM com DB:** injetar `DATABASE_URL=postgresql://ffg:<senha-urlencoded>@127.0.0.1:<port>/litellm?sslmode=disable`; manter `LITELLM_SALT_KEY` estável; aguardar readiness; primeiras migrações Prisma podem demorar.
6. **Credential:** criar `github-copilot` via API de management (`POST /credentials`) se a versão fixada suportar; validar o schema exato no spike. Fallback: abrir a UI do LiteLLM em `WebviewWindow` na página de credenciais; se a UI exigir login, resolver com sessão local de curta duração sem expor a master key (spike obrigatório).
7. **OAuth/device flow:** preferir navegador do sistema para `verification_uri`; mostrar `user_code`, link e expiração; polling a cada 5 s; timeout de 10 min; cancelar; nunca logar token.
8. **Aliases:** gerar `config.yaml` com `claude-opus-4-8` e `claude-haiku-4-5` apontando para `github_copilot/<modelo-validado>` e `litellm_credential_name: github-copilot`; fallback configurável.
9. **Teste:** `POST /v1/messages` com `max_tokens: 8`; sucesso → `connected`; 401 → reconectar; 403/404 de modelo → “modelo não disponível no seu plano”; 429 → aguardar.
10. **Troca de perfil:** ao sair do Copilot, parar Postgres, remover `DATABASE_URL`, regenerar config e reiniciar LiteLLM; manter dados para reconectar, salvo “apagar dados”.

Segurança: o banco contém tokens OAuth; data dir com ACL do usuário; backup só com app parado e protegido; nunca incluir `DATABASE_URL` em logs/eventos. O uso do Copilot via OAuth deve passar por revisão de ToS antes de release público.

### 6.4 Geração do `config.yaml` e restart do LiteLLM

1. `ffg-core::litellm_config` valida rotas, fallbacks, nomes públicos e ausência de segredos.
2. Gera YAML determinístico; app valida parse e grava candidato atômico.
3. Para LiteLLM atual com graceful stop; aguarda porta liberar; inicia novo processo com env mínimo (master key, salt key, provider keys, `DATABASE_URL` se Copilot, proxy corporativo original se existir).
4. Aguarda `/health/liveliness` e `/health/readiness` com timeout de 60–90 s; registra log tail em caso de falha.
5. Testa cada rota pública; se falhar, restaura `config.yaml` anterior e reinicia.
6. Só então atualiza `setup.json` (`configRevision`, `appliedAt`) e emite `setup://state`.
7. Histórico mantém as últimas N configs; rollback manual disponível no dashboard.

### 6.5 Instalação/remoção da integração Figma

**Instalação:**

1. Preflight: Windows, Figma detectado, portas, política de CA, proxy preexistente; exigir escolha se houver proxy.
2. Gerar/carregar CA raiz e leaf para `api.figma.com` (e hosts configurados); instalar CA em `CurrentUser\Root` via WinCrypt; registrar thumbprint.
3. Subir o proxy em `127.0.0.1:<porta>`; health local.
4. Snapshot do env (`HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY`, `ALL_PROXY`) no Credential Manager/DPAPI; journal.
5. Gravar env em `HKCU\Environment` com `RegSetValueExW`; `NO_PROXY` inclui `localhost,127.0.0.1,::1` e preserva entradas existentes; broadcast `WM_SETTINGCHANGE`.
6. Validar:
   - TCP + TLS handshake com SNI `api.figma.com` usando a CA local;
   - `GET /ai-gateway/llm/provider/universal/health/liveliness` through proxy → rewrite + auth + LiteLLM 200 (sem custo de modelo);
   - `HEAD /` through proxy → forward intacto para a Figma (não 502), provando passthrough;
   - opcional: `POST /v1/messages` com `max_tokens` mínimo, se o provider já estiver configurado.
7. Detectar Figma aberto; oferecer “Fechar e reabrir” com aviso de trabalho não salvo, ou “depois”.
8. Marcar verde quando a primeira requisição de IA real passar (`proxy://first-ai-request`).

**Remoção:**

1. Restaurar snapshot do env exatamente (valor anterior ou remoção da variável).
2. Broadcast.
3. Parar proxy.
4. Remover apenas a CA com o thumbprint nosso em `CurrentUser\Root`.
5. Verificar ausência; limpar journal; avisar reinício do Figma.

**Rollback:** qualquer falha após a instalação da CA desfaz env → proxy → CA. A remoção é idempotente. Ao “Sair” do app, remover o env (para o Figma não apontar para proxy morto), manter a CA e avisar; no próximo start, reaplicar se a integração estava ativa. Se o app crashar, o próximo start detecta env órfão e repara/avisa.

### 6.6 Teste ponta a ponta

`e2e_test` executa e retorna `E2ETestReport`:

1. runtime ativo e `--version` correto;
2. Postgres saudável (se Copilot);
3. LiteLLM liveness/readiness;
4. config aplicada e modelos listados;
5. teste de cada rota pública com `max_tokens: 8` (ou `count_tokens`);
6. proxy saudável;
7. CA instalada e confiável;
8. env presente;
9. TLS + rewrite via proxy (`health/liveliness`);
10. passthrough não-IA (`HEAD /`);
11. primeira requisição real do Figma (pode ficar “aguardando” até o usuário testar no Make);
12. Figma detectado/versão.

O relatório não inclui prompt/resposta; só status, duração e erro sanitizado. O teste real com provider é opcional no finish; em CI usa fake provider/fake LiteLLM.

---

## 7. Fases de implementação

### F0 — Fundação: estado, paths, segredos, contratos e shell tipado

**Objetivo:** congelar os contratos transversais para que as fases seguintes não redescubram tipos, caminhos, erros e eventos.

**Depende de:** nada.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/Cargo.toml` | alterar | Preparar workspace para futuros membros `crates/runtime`, `crates/proxy` |
| `src/crates/core/src/lib.rs` | alterar | Exportar `setup`, `error`, `checks`, `redaction` |
| `src/crates/core/src/setup.rs` | criar | `SetupState`, `StepId`, `StepStatus`, transições, `first_pending_step`, `reset_scope`, migração |
| `src/crates/core/src/error.rs` | criar | `UiError`, `ErrorCode`, helpers de sanitização |
| `src/crates/core/src/checks.rs` | criar | Política pura de checks de ambiente |
| `src/crates/core/src/redaction.rs` | criar | Redaction de headers/URL/body/segredos |
| `src/src-tauri/src/state.rs` | criar | `AppState` com locks async, paths, stores, managers |
| `src/src-tauri/src/paths.rs` | criar | `RuntimePaths` a partir de `app_local_data_dir` |
| `src/src-tauri/src/events.rs` | criar | Constantes e payloads de eventos |
| `src/src-tauri/src/setup_store.rs` | criar | Load/save atômico, `.bak`, quarentena, revisão |
| `src/src-tauri/src/secrets.rs` | criar | Trait `SecretStore`, `SecretKey`, `SecretValue`, adapter `keyring` e mock de teste |
| `src/src-tauri/src/commands.rs` | alterar | Dividir em `commands/mod.rs` + submódulos vazios |
| `src/src-tauri/src/commands/{app,setup,services}.rs` | criar | Comandos existentes e de setup |
| `src/src-tauri/src/lib.rs` | alterar | Registrar `AppState`, setup, comandos e eventos |
| `src/src-tauri/Cargo.toml` | alterar | `tokio`, `serde_json`, `uuid`, `thiserror`, `keyring`, `secrecy`, `zeroize` |
| `src/ui/src/lib/contracts.ts` | criar | Tipos TS espelhando Rust |
| `src/ui/src/lib/bridge.ts` | alterar | Wrappers tipados; remover strings soltas |
| `src/ui/src/state/AppStore.tsx` | criar | Contexto/reducer, assinatura de eventos, `screen` |
| `src/ui/src/screens/Dashboard.tsx` | criar | Extrair dashboard atual de `App.tsx` |
| `src/ui/src/App.tsx` | alterar | Shell com `screen` e provider do store |
| `docs/adr/0002-setup-state.md` | criar | Registrar schema/versionamento/retomada |

**Comandos/eventos:** `setup_get_state`, `setup_begin_step`, `setup_complete_step`, `setup_set_options`, `setup_reset`, `operation_cancel`; evento `setup://state`.

**Fora de escopo:** download real, processos, CA/proxy, UI do wizard.

**Critérios de aceite:**

- `cargo fmt --all -- --check`, clippy e `cargo test -p ffg-core` verdes.
- `setup.json` round-trip com fixture; corrupção cai para `.bak` e quarentena.
- Migração de versão de schema testada.
- Nenhum segredo em `setup.json` (teste com serialização).
- `pnpm build` verde; dashboard atual funciona via store tipado.
- Evento `setup://state` entrega revisão monotônica.

**Testes:** unitários de transição, serialização, migração, corrupção, redaction básica; teste de contrato TS/Rust para nomes de campos.

**Esforço:** 1–2 semanas.

---

### F1 — Núcleo puro: manifesto assinado, download/install plan, config, providers e journal

**Status:** implementado no `ffg-core` (ver [`CHANGELOG.md`](CHANGELOG.md)); I/O real fica em F2/F3/F9.

**Objetivo:** implementar toda a lógica que pode ser testada sem Windows, rede ou Tauri.

**Depende de:** F0.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/crates/core/src/manifest.rs` | alterar | Estender `RuntimeManifest`, `ComponentManifest` (`installedSize`, `entrypoint`, `versionArgs`), assinatura Ed25519 com janela de validade, `verify_manifest`, `RuntimePolicy` |
| `src/crates/core/src/download.rs` | criar | `DownloadPlan`, `ResumeState`, `DownloadAction`, `RetryPolicy`, `MirrorPlan`, erros puros |
| `src/crates/core/src/install.rs` | criar | `InstallPlan`, validação de path/zip, layout esperado, `ActiveRuntime`, rollback |
| `src/crates/core/src/providers.rs` | criar | Catálogo, `ProviderDescriptor`, `ProviderProfileInput`, validação de campos e secret refs (reusa `ProviderKind` do setup) |
| `src/crates/core/src/litellm_config.rs` | criar | `generate_litellm_config`, validação de rotas/fallbacks e YAML determinístico |
| `src/crates/core/src/env_plan.rs` | criar | `ProxyEnvPlan`, `EnvSnapshot`, merge de `NO_PROXY`, restore |
| `src/crates/core/src/integration.rs` | criar | `IntegrationJournal`, ordem de passos, rollback, status, idempotência |
| `src/crates/core/src/copilot.rs` | criar | `DeviceFlow`, polling/timeout, transições de auth e mapeamento de erros (reusa `CopilotState`/`AuthStatus`) |
| `src/crates/core/src/models.rs` | alterar | Validação de nomes públicos, rotas, fallback sem ciclo |
| `src/crates/core/src/redaction.rs` | alterar | Casos de headers/URL/body; fixtures |
| `src/crates/core/Cargo.toml` | alterar | `ed25519-dalek`, `base64`, `semver`, `serde_yaml` |
| `src/crates/core/tests/fixtures/` | criar | Manifesto de exemplo e YAML golden do LiteLLM |

**Interfaces principais:**

```rust
pub fn verify_manifest(
    raw_manifest: &[u8],
    signature: &ManifestSignature,
    trusted_keys: &[TrustedKey],
    app_version: &str,
    policy: &RuntimePolicy,
    now_epoch_seconds: u64,
) -> Result<RuntimeManifest, ManifestError>;

pub fn plan_download_action(
    resume: Option<&ResumeState>,
    expected_size: u64,
) -> DownloadAction;

impl DownloadPlan {
    pub fn new(
        component: ComponentId,
        manifest: &ComponentManifest,
        mirrors: Vec<String>,
        resume: Option<&ResumeState>,
    ) -> Result<Self, DownloadError>;
}

pub fn generate_litellm_config(settings: &ModelSettings) -> Result<String, ConfigError>;

pub fn plan_proxy_env(
    current: &EnvSnapshot,
    desired: &ProxyDesired,
) -> Result<ProxyEnvPlan, EnvError>;
```

**Fora de escopo:** I/O de arquivo, HTTP, processos, Win32.

**Critérios de aceite:**

- `cargo llvm-cov -p ffg-core --fail-under-lines 80` verde.
- Assinatura válida aceita; bytes alterados, chave desconhecida, chave revogada, chave fora da
  janela de validade e `minAppVersion` maior são rejeitados.
- YAML golden para cada provider, com aliases e fallback; nenhum segredo no output.
- Plano de download cobre 206/200/416/ETag/If-Range.
- Plano de instalação rejeita path traversal, symlink e zip bomb.
- Env plan restaura exatamente valores anteriores, incluindo ausência.
- Journal rollback na ordem inversa.
- `cargo clippy -p ffg-core --all-targets -- -D warnings`.

**Testes:** unitários para env/redaction/paths; fixtures de assinatura geradas com chave de teste em runtime; sem rede.

**Esforço:** 2–3 semanas.

---

### F2 — Artefatos de runtime e CD mínimo (ZIPs + manifesto assinado)

**Status:** workflow, scripts, gerador e ferramentas criados; build real e publicação pendentes de
validação no Windows.

**Objetivo:** produzir os artefatos reais que o downloader e o onboarding consomem.

**Depende de:** F1 (formato do manifesto).

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `.github/workflows/release.yml` | criar | Tag `v*`: build LiteLLM, Postgres, manifesto, assinatura, release, smoke Windows |
| `.github/workflows/ci.yml` | alterar | Jobs para novos crates; manter gate 80% do core |
| `scripts/build-litellm-runtime.ps1` | criar | PyInstaller one-folder ou venv selado; Python fixado; `litellm --version`; smoke com Postgres; excluir `enterprise/` |
| `scripts/build-pgsql-runtime.ps1` | criar | ZIP do Postgres 16.x oficial; `postgres --version`; licença |
| `scripts/manifest.mjs` | criar | Gerar `manifest.json` com versões, URLs, SHA-256, size, installedSize, entrypoint, minAppVersion |
| `src/tools/manifest-signer/` | criar | Binário Rust Ed25519 para assinar `manifest.json` (CI/local) |
| `src/tools/manifest-verify/` | criar | Verificador de assinatura/hash para CI e diagnóstico |
| `src/src-tauri/resources/keys/manifest-keys.json` | criar | Chaves públicas embutidas, `keyId`, validade/status |
| `src/src-tauri/resources/runtime-policy.json` | criar/gerar | LiteLLM permitido `1.108.1`; Postgres major 16 |
| `docs/release.md` ou atualização de `docs/dev-guide.md` | alterar | Como gerar/assinar/verificar artefatos |

**Fora de escopo:** updater do app, assinatura de código do instalador (F11), espelho S3 (pode ficar para F11).

**Critérios de aceite:**

- Workflow de tag gera `litellm-runtime-1.108.1-win-x64.zip`, `pgsql-16.x-win-x64.zip`, `manifest.json`, `manifest.json.sig`.
- `manifest-verify` aceita o artefato e rejeita 1 byte alterado.
- Smoke em runner Windows: `litellm --version` e LiteLLM sobe com config mínima + Postgres fake/real.
- Nenhuma referência a `enterprise/`; notices/licenças atualizados.
- Artefatos publicados com SHA-256 e tamanhos; release tem rollback para versão anterior.

**Testes:** pipeline dry-run; verificação de assinatura com chave de teste; smoke Windows; checagem de hash; inspeção de conteúdo do ZIP.

**Esforço:** 2–3 semanas (inclui spike PyInstaller vs venv selado).

---

### F3 — Downloader e instalação atômica (`ffg-runtime`)

**Status:** crate, testes e comandos Tauri implementados; E2E com artefatos reais pendente no Windows.

**Objetivo:** baixar, pausar/retomar, verificar, extrair, validar e ativar runtimes com rollback.

**Depende de:** F1; F2 para teste real (pode usar fake artifacts antes).

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/crates/runtime/Cargo.toml` | criar | Dependências `reqwest` (rustls), `tokio`, `zip`, `sha2`, `serde`, `tempfile`, `fs2`, `tokio-util` |
| `src/crates/runtime/src/lib.rs` | criar | API pública `RuntimeClient` |
| `src/crates/runtime/src/download.rs` | criar | Orquestração de download/resume/progresso |
| `src/crates/runtime/src/http.rs` | criar | Cliente HTTP direto/Proxy externo, Range, retries, timeouts |
| `src/crates/runtime/src/resume.rs` | criar | `.part` + `.meta.json`, ETag, Last-Modified, locking |
| `src/crates/runtime/src/verify.rs` | criar | Hash streaming, tamanho, integração com manifesto |
| `src/crates/runtime/src/extract.rs` | criar | ZIP seguro, limites, staging |
| `src/crates/runtime/src/install.rs` | criar | Ativação atômica, histórico, rollback |
| `src/crates/runtime/src/active.rs` | criar | `active.json` e versão anterior |
| `src/crates/runtime/src/probe.rs` | criar | Probe `--version` com trait para fake |
| `src/crates/runtime/src/offline.rs` | criar | Import local com manifesto/assinatura |
| `src/crates/runtime/src/error.rs` | criar | `RuntimeError` mapeável para `UiError` |
| `src/Cargo.toml` | alterar | Adicionar membro |
| `src/src-tauri/src/runtime_manager.rs` | criar | Adaptador Tauri, paths, eventos, cancelamento |
| `src/src-tauri/src/commands/runtime.rs` | criar | Comandos `runtime_*` |
| `src/src-tauri/Cargo.toml` | alterar | Dependências de runtime/eventos |

**Comandos/eventos:** `runtime_status`, `runtime_download`, `runtime_pause`, `runtime_resume`, `runtime_cancel`, `runtime_import_offline`, `runtime_activate`; eventos `download://progress`, `download://state`.

**Fora de escopo:** iniciar o LiteLLM (F4), UI do passo 1 (F10), updater.

**Critérios de aceite:**

- Em Windows, baixar o artefato real de F2, pausar em ~50%, fechar o app, reabrir e retomar sem corromper.
- Hash inválido nunca extrai; assinatura inválida aborta antes de baixar.
- ZIP com path traversal/symlink é rejeitado.
- `--version` divergente não ativa.
- Falha no meio da ativação mantém a versão anterior funcional.
- Import offline instala o mesmo artefato com manifesto assinado.
- Limpeza de staging órfão na inicialização.
- Nenhum segredo em log/evento.

**Testes:** fake HTTP server (206, 200, 416, ETag, truncado, lento, 500, cancelamento), resume após “restart”, zip-slip/zip bomb, disco cheio simulado, probe fake, rollback, import offline, lock concorrente.

**Esforço:** 2–3 semanas.

---

### F4 — Supervisor de sidecars, Job Object, health, portas e logs

**Status:** implementado no crate `ffg-supervisor` e ligado aos comandos do app; Job Object/processos
reais pendentes de validação no Windows.

**Objetivo:** ciclo de vida confiável de Postgres/LiteLLM/proxy.

**Depende de:** F3 (runtime instalado; pode usar fake até F3 terminar).

> Implementação: a supervisão foi extraída para `crates/supervisor` (`ffg-supervisor`) para ser
> testável no Linux; o app registra os specs e consome os comandos.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/src-tauri/src/supervisor.rs` | alterar | Reexportar módulo `supervisor/` |
| `src/src-tauri/src/supervisor/mod.rs` | criar | `Supervisor`, desired state, eventos |
| `src/src-tauri/src/supervisor/process.rs` | criar | Spawn, stdout/stderr, timeout, kill tree |
| `src/src-tauri/src/supervisor/health.rs` | criar | Probes TCP/HTTP, deadlines, estados |
| `src/src-tauri/src/supervisor/ports.rs` | criar | Alocação/persistência de portas loopback |
| `src/src-tauri/src/supervisor/backoff.rs` | criar | Backoff exponencial com jitter |
| `src/src-tauri/src/supervisor/job.rs` | criar | Job Object Windows (`KILL_ON_JOB_CLOSE`) |
| `src/src-tauri/src/supervisor/logs.rs` | criar | Ring buffer, arquivos rotativos, redaction |
| `src/src-tauri/src/platform/mod.rs` | criar | Traits `ProcessGroup`, `ProcessInspector`, etc. |
| `src/src-tauri/src/platform/windows/job.rs` | criar | Implementação Win32 encapsulada |
| `src/src-tauri/src/platform/dev/process_group.rs` | criar | Fallback de teste |
| `src/src-tauri/src/startup.rs` | criar | Orquestração de startup/shutdown |
| `src/src-tauri/src/commands/services.rs` | criar | `service_status`, `start_services`, `stop_services`, `restart_services`, `service_logs` |
| `src/src-tauri/Cargo.toml` | alterar | `tokio`, `tokio-util`, `windows`, `sysinfo` |
| `src/tools/test-child/` | criar | Binário de teste que dorme/crasha/ecoa logs |

**Comandos/eventos:** comandos de serviços; evento `service://status`.

**Fora de escopo:** init do Postgres (F9), proxy (F7), UI do dashboard (F10).

**Critérios de aceite:**

- Dashboard inicia/para/reinicia LiteLLM.
- Matar LiteLLM externamente dispara restart com backoff; após N falhas, `failed` + notificação.
- Sair do app mata todos os filhos; nenhum órfão.
- Porta ocupada → nova porta persistida; LiteLLM sobe nela.
- Logs com headers sensíveis redigidos; rotação funciona.
- Health timeout não trava a UI.
- `cargo clippy` verde no Windows.

**Testes:** fake child, crash/restart, backoff, port conflict, shutdown, log redaction; teste Windows de Job Object (child morto ao fechar o job).

**Esforço:** 2 semanas.

---

### F5 — Configuração do LiteLLM, providers por API key e teste transacional

**Status:** config store, client e comandos de providers/modelos implementados; teste transacional
com uma instância LiteLLM real e Credential Manager pendentes de validação no Windows.

**Objetivo:** entregar o backend do passo 2 (exceto Copilot): catálogo, credenciais, config, aplicação e teste.

**Depende de:** F1, F4.

> Implementação: config store e client foram extraídos para `crates/litellm` (`ffg-litellm`);
> o `src-tauri` mantém a orquestração, segredos e comandos.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/src-tauri/src/litellm/mod.rs` | criar | Fachada do LiteLLM |
| `src/src-tauri/src/litellm/client.rs` | criar | Cliente management: health, messages test, models |
| `src/src-tauri/src/litellm/config_store.rs` | criar | Candidato/atual/histórico, escrita atômica, validação YAML |
| `src/src-tauri/src/litellm/lifecycle.rs` | criar | Start/stop/restart com env e timeout |
| `src/src-tauri/src/litellm/test.rs` | criar | Teste transitório de provider |
| `src/src-tauri/src/providers/mod.rs` | criar | Orquestração de perfis |
| `src/src-tauri/src/providers/commands.rs` | criar | Comandos `provider_*` |
| `src/src-tauri/src/providers/secrets.rs` | criar | Mapeamento provider → env/secret ref |
| `src/src-tauri/src/commands/providers.rs` | criar | Registro dos comandos |
| `src/src-tauri/resources/providers.json` | criar | Catálogo/IDs recomendados versionados (sem segredos) |
| `src/tools/fake-litellm/` | criar | Servidor fake para testes (health, `/v1/messages`, erros) |
| `src/src-tauri/Cargo.toml` | alterar | `reqwest`, `serde_yaml`, `url` |

**Comandos/eventos:** `providers_catalog`, `provider_test`, `provider_save`, `provider_remove`, `provider_secret_status`; `operation://progress` para operações longas.

**Fora de escopo:** Copilot/Postgres (F9), UI completa (F10).

**Critérios de aceite:**

- Perfil OpenRouter/Anthropic/OpenAI/Gemini/Azure/local pode ser salvo e testado.
- Chave errada retorna erro claro (401) sem gravar segredo inválido.
- Falha ao aplicar config restaura a anterior e reinicia o LiteLLM.
- Nenhuma API key no `config.yaml`, `setup.json`, logs ou eventos.
- Aliases públicos mantidos; fallback validado.
- Teste de conexão não usa adapter próprio.

**Testes:** golden configs; fake LiteLLM; rollback transacional; keyring mock; redaction; teste de erro 401/404/429/timeout; teste de que o LiteLLM não herda nosso proxy.

**Esforço:** 2–3 semanas.

---

### F6 — CA local, proteção da chave e trust store do Windows

**Status:** CA, DPAPI/cert store e comandos implementados; instalação real no Windows pendente de
validação.

**Objetivo:** gerar/gerenciar a CA e instalá-la/removê-la em `CurrentUser\Root` sem admin.

**Depende de:** F1.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/crates/proxy/Cargo.toml` | criar | `rcgen`, `rustls`, `x509-parser`, `sha1`, `time` |
| `src/crates/proxy/src/lib.rs` | criar | API pública da CA |
| `src/crates/proxy/src/ca/mod.rs` | criar | `CertificateAuthority` |
| `src/crates/proxy/src/ca/root.rs` | criar | Geração/carregamento da raiz, validade, key usage |
| `src/crates/proxy/src/ca/leaf.rs` | criar | Leaf por host/SAN, cache, renovação |
| `src/crates/proxy/src/ca/cert.rs` | criar | PEM/DER, thumbprint SHA-1, serialização |
| `src/src-tauri/src/platform/cert_store.rs` | criar | Trait `CertStore` |
| `src/src-tauri/src/platform/windows/cert_store.rs` | criar | WinCrypt: install/remove/is_installed por thumbprint |
| `src/src-tauri/src/platform/dev/cert_store.rs` | criar | Implementação em memória para testes |
| `src/src-tauri/src/platform/file_protector.rs` | criar | Trait `FileProtector` |
| `src/src-tauri/src/platform/windows/dpapi.rs` | criar | `CryptProtectData`/`CryptUnprotectData` |
| `src/src-tauri/src/ca/mod.rs` | criar | Manager: ensure/load/export/status |
| `src/src-tauri/src/commands/ca.rs` | criar | `ca_status`, `ca_install`, `ca_remove`, `ca_export` |
| `src/Cargo.toml` | alterar | Adicionar membro proxy |

**Comandos/eventos:** `ca_*`; `integration://status` parcial.

**Fora de escopo:** servidor proxy MITM (F7), env/Figma (F8).

**Critérios de aceite:**

- CA raiz gerada com `CA:TRUE`, `keyCertSign`, `cRLSign`; chave privada apenas em arquivo DPAPI com ACL.
- Leaf com SANs corretos para `api.figma.com` e variantes configuradas; validade curta; renovação antes de expirar.
- `ca_install` instala em `CurrentUser\Root`; `certutil -user -store Root` mostra o thumbprint.
- `ca_remove` remove apenas a nossa CA; é idempotente.
- Cliente TLS de teste confia na raiz e valida a leaf.
- Política corporativa bloqueando a instalação produz erro claro + exportação manual.
- Sem admin.

**Testes:** geração/validação de cadeia, thumbprint, instalação/remoção no Windows, política negada, export, DPAPI round-trip, permissões de arquivo.

**Esforço:** 1–2 semanas.

---

### F7 — Proxy MITM: túnel, MITM seletivo, rewrite, auth e streaming

**Status:** servidor, self-test e comandos implementados; validação com o Figma real e casos
HTTP/2/Upgrade pendentes no Windows.

**Objetivo:** interceptar somente a inferência da Figma e encaminhar o resto intacto.

**Depende de:** F1 (rewrite), F6 (CA).

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/crates/proxy/src/server.rs` | criar | Listener HTTP proxy, CONNECT, accept loop |
| `src/crates/proxy/src/connect.rs` | criar | Parsing `host:port`, túnel raw, loopback bypass |
| `src/crates/proxy/src/mitm.rs` | criar | Terminação TLS seletiva por host, SNI, leaf |
| `src/crates/proxy/src/route.rs` | criar | Classificação host/path → Tunnel/Forward/AiGateway |
| `src/crates/proxy/src/upstream.rs` | criar | Forward não-IA e chamada ao LiteLLM; headers |
| `src/crates/proxy/src/stream.rs` | criar | SSE/streaming, backpressure, cancelamento |
| `src/crates/proxy/src/config.rs` | criar | `ProxyConfig`, hosts MITM, timeouts, modos |
| `src/crates/proxy/src/health.rs` | criar | `GET /__ffg/health` local |
| `src/crates/proxy/src/stats.rs` | criar | Conexões, requisições IA, primeira requisição, erros |
| `src/crates/proxy/src/error.rs` | criar | Erros de TLS/upstream/parse |
| `src/src-tauri/src/proxy/mod.rs` | criar | Fachada/controle do proxy |
| `src/src-tauri/src/proxy/controller.rs` | criar | Start/stop/restart, porta, status, evento first-ai |
| `src/src-tauri/src/commands/proxy.rs` | criar | `proxy_status`, `proxy_self_test` |
| `src/src-tauri/Cargo.toml` | alterar | `hyper`, `hyper-util`, `http-body-util`, `tokio-rustls`, `rustls-native-certs`, `bytes`, `http` |
| `src/crates/proxy/tests/fixtures/` | criar | Requests/respostas redigidas, SSE, count_tokens |

**Comandos/eventos:** `proxy_status`, `proxy_self_test`; `proxy://first-ai-request`; `service://status` para o proxy.

**Decisão de stack:** spike timeboxed de 3–5 dias comparando `hudsucker` com implementação minimal hyper + rustls. Recomendação inicial: hyper + rustls para controlar MITM seletivo, streaming e Upgrade; se `hudsucker` reduzir risco sem perder controle, usar e isolar atrás da mesma API. Registrar ADR.

**Fora de escopo:** instalar CA/env (F6/F8), reiniciar Figma (F8), fail-open completo.

**Critérios de aceite:**

- CONNECT para `www.figma.com`, `mcp.figma.com`, `desktop.figma.com`, localhost e hosts desconhecidos → túnel raw, sem MITM.
- `api.figma.com/ai-gateway/.../v1/messages` → MITM + rewrite + `Authorization: Bearer <master>` + LiteLLM.
- `api.figma.com/qualquer-outra-rota` → MITM + forward intacto, preservando auth do Figma e corpo.
- SSE repassado sem buffering; primeiro byte dentro do alvo; 4 streams concorrentes.
- Cancelamento do cliente encerra upstream.
- HTTP/2 e HTTP Upgrade/WebSocket cobertos ou explicitamente tunnelados; testes.
- `health/liveliness` via rewrite funciona sem custo de modelo.
- p95 de overhead ≤50 ms no primeiro byte (ambiente local).
- Nenhum token de Figma chega ao LiteLLM; nenhuma master key chega ao upstream não-IA.
- Logs sem prompt/corpo.

**Testes:** fake upstream TLS + fake LiteLLM; matriz de rotas; SSE chunk timing; h2; upgrade; erros 401/429/500; timeout; cliente desconecta; proxy não faz loop para si; tráfego não-IA byte-equivalente (exceto hop-by-hop).

**Esforço:** 4–6 semanas. É o maior risco técnico do projeto.

---

### F8 — Integração Figma: env HKCU, detecção, instalação/remoção e validação

**Objetivo:** instalar a integração com um clique, validar e reverter sem admin.

**Depende de:** F4 (supervisor), F6 (CA), F7 (proxy).

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/src-tauri/src/platform/env_store.rs` | criar | Trait `EnvStore`, `EnvSnapshot` |
| `src/src-tauri/src/platform/windows/env_store.rs` | criar | `RegQueryValueExW`/`RegSetValueExW`, broadcast |
| `src/src-tauri/src/platform/dev/env_store.rs` | criar | Fake para testes |
| `src/src-tauri/src/figma/mod.rs` | criar | Fachada |
| `src/src-tauri/src/figma/detect.rs` | criar | Estratégias de detecção/versão |
| `src/src-tauri/src/figma/process.rs` | criar | Processo em execução, `WM_CLOSE`, força |
| `src/src-tauri/src/figma/restart.rs` | criar | Fechar/reabrir com confirmação |
| `src/src-tauri/src/integration/mod.rs` | criar | Orquestração instalar/remover/status |
| `src/src-tauri/src/integration/journal.rs` | criar | Persistência do journal, rollback |
| `src/src-tauri/src/integration/validate.rs` | criar | TLS SNI, rewrite, passthrough, first-ai |
| `src/src-tauri/src/commands/integration.rs` | criar | `figma_*`, `integration_status` |
| `src/src-tauri/src/tray.rs` | alterar | Menu “Reconfigurar”, “Configurar Figma”, “Sair” com cleanup |
| `src/src-tauri/src/lib.rs` | alterar | Startup repair, quit behavior |
| `src/src-tauri/Cargo.toml` | alterar | `windows`, `sysinfo` |

**Comandos/eventos:** `figma_environment`, `figma_install`, `figma_validate`, `figma_remove`, `figma_restart`, `integration_status`; eventos `integration://status`, `proxy://first-ai-request`, `wizard://open`.

**Fora de escopo:** Copilot (F9), UI do passo 4 (F10).

**Critérios de aceite:**

- Instalação sem admin: CA em `CurrentUser\Root`, env em `HKCU\Environment`, proxy em loopback.
- `reg query HKCU\Environment` mostra os valores; `NO_PROXY` preserva entradas.
- Validação TLS + rewrite (`health/liveliness`) e passthrough não-IA passam.
- Primeira requisição real de IA marca verde no dashboard.
- Remoção restaura exatamente o env anterior (incluindo ausência) e remove só a nossa CA.
- Proxy preexistente não é sobrescrito sem confirmação; snapshot/restore funciona.
- Figma aberto: opção “Fechar e reabrir” com aviso; “depois” mantém estado pendente.
- Sair do app remove env e avisa; startup repara env órfão.
- Rollback funciona em falha simulada no meio.

**Testes:** env store (mock + Windows registry), merge/restore, journal rollback, detecção com fixtures de pastas/registry, restart manual, validação TLS/rewrite, regressão não-IA, instalação/remoção idempotente.

**Esforço:** 2–3 semanas.

---

### F9 — Postgres embutido + GitHub Copilot (OAuth, credential, alias e teste)

**Objetivo:** fechar o sub-fluxo Copilot completo.

**Depende de:** F4, F5. Pode rodar em paralelo com F6–F8.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/src-tauri/src/postgres/mod.rs` | criar | Fachada |
| `src/src-tauri/src/postgres/manager.rs` | criar | Ciclo de vida, porta, status |
| `src/src-tauri/src/postgres/init.rs` | criar | `initdb`, database, role, senha |
| `src/src-tauri/src/postgres/backup.rs` | criar | Backup/restore com app parado |
| `src/src-tauri/src/copilot/mod.rs` | criar | Fachada/estado |
| `src/src-tauri/src/copilot/manager.rs` | criar | Prepare/auth/disconnect/profile switch |
| `src/src-tauri/src/copilot/credential.rs` | criar | `POST /credentials` e fallback UI |
| `src/src-tauri/src/copilot/device_flow.rs` | criar | Device flow/poll/timeout |
| `src/src-tauri/src/copilot/test.rs` | criar | Teste de alias e mapeamento de erros |
| `src/src-tauri/src/commands/copilot.rs` | criar | Comandos `copilot_*` |
| `src/src-tauri/src/litellm/client.rs` | alterar | Credentials/models/test |
| `src/src-tauri/Cargo.toml` | alterar | `url`, `secrecy`, `zeroize` |

**Comandos/eventos:** `copilot_status`, `copilot_prepare`, `copilot_begin_auth`, `copilot_poll_auth`, `copilot_disconnect`, `copilot_disable`; evento `copilot://auth`.

**Fora de escopo:** providers API key (F5), UI (F10).

**Critérios de aceite:**

- Postgres baixa/inicializa/sobe/para sem admin, em `127.0.0.1`, com senha no Credential Manager.
- LiteLLM sobe com `DATABASE_URL`, readiness OK, migrações concluídas.
- Credential `github-copilot` criada; device flow mostra código/link e conclui; “Connected”.
- Aliases Copilot aplicados e teste `/v1/messages` OK.
- 401 → reconectar; modelo fora do plano → mensagem clara; 429 → aguardar.
- Troca de perfil Copilot → leve para Postgres, remove `DATABASE_URL`, regenera config.
- Nenhum token em log/evento/JSON.
- Backup/restore do data dir documentado e testado.

**Testes:** init/start/stop/readiness com binários reais no Windows; fake management API; device flow states; timeout/cancel; erros 401/403/404/429; profile switch; backup; corrupção de data dir; redaction de `DATABASE_URL`.

**Esforço:** 3–4 semanas.

---

### F10 — Wizard completo, dashboard, tray e acessibilidade

**Objetivo:** entregar a experiência de onboarding dos 5 passos, com retomada, reconfiguração e modo offline.

**Depende de:** F3, F5, F8, F9 (shell/UI pode começar antes com bridge mockada).

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/ui/src/App.tsx` | alterar | Shell com `screen` dashboard/wizard/providers/logs/settings |
| `src/ui/src/state/AppStore.tsx` | alterar | Estado, eventos, operações, toasts |
| `src/ui/src/lib/bridge.ts` | alterar | Todos os wrappers |
| `src/ui/src/lib/contracts.ts` | alterar | Tipos completos |
| `src/ui/src/lib/events.ts` | alterar | Listeners tipados |
| `src/ui/src/screens/Dashboard.tsx` | alterar | Cards de serviços/integração, ações |
| `src/ui/src/screens/wizard/*` | criar | `WizardModal` + carrossel + 6 conteúdos de passo |
| `src/ui/src/screens/wizard/wizard.css` | criar | Modal, carrossel, responsivo e reduced motion |
| `src/ui/src/screens/providers/*` | criar | Edição pós-onboarding |
| `src/ui/src/screens/logs/*` | criar | Tail/filtro/copiar |
| `src/ui/src/screens/settings/*` | criar | Autostart, pastas, reset |
| `src/ui/src/components/*` | criar | Progress, Alert, ConfirmDialog, Field, etc. |
| `src/ui/src/styles.css` | alterar | Layout, foco, contraste, responsivo |
| `src/ui/src/test/*` | criar | Vitest/RTL/axe |
| `src/package.json` | alterar | `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener`, Vitest/RTL/axe |
| `src/vite.config.ts` | alterar | Config de teste |
| `src/src-tauri/tauri.conf.json` | alterar | Tamanho da janela, CSP, bundle |
| `src/src-tauri/capabilities/default.json` | alterar | Permissões de dialog/opener/autostart |
| `src/src-tauri/Cargo.toml` | alterar | `tauri-plugin-dialog`, `tauri-plugin-opener` |
| `src/src-tauri/src/tray.rs` | alterar | Itens e evento `wizard://open` |
| `src/src-tauri/src/lib.rs` | alterar | Registro dos plugins/comandos |

**Comandos/eventos:** todos os `setup_*`, `runtime_*`, `provider_*`, `copilot_*`, `models_*`, `figma_*`, `ca_*`, `service_*`, `diagnostics_*`; eventos da seção 4.4.

**Fora de escopo:** novos recursos de backend; i18n.

**Critérios de aceite:**

- Wizard completo em Windows, em modal overlay com carrossel horizontal, first-run de ponta a ponta.
- Fechar/reabrir retoma no passo pendente; fechar não cancela download/OAuth em andamento.
- `Esc`, botão de fechar e clique no backdrop fecham o modal; foco preso e restaurado ao elemento de origem.
- Tray “Reconfigurar” abre no passo 2; “Configurar Figma” no passo 4.
- “Já tenho os pacotes” instala offline.
- Pular Figma mostra badge pendente no dashboard.
- Erros têm mensagem/ação; download pode pausar/retomar/cancelar.
- Providers/modelos podem ser editados depois do onboarding.
- Testes de UI verdes; `pnpm build`; axe sem violações críticas.
- Navegação por teclado, foco, labels, `aria-live` e contraste.
- Nenhum `any`; nenhum segredo na UI.

**Testes:** Vitest/RTL para todos os passos, resume, offline, skip, erro/retry, rollback; axe; bridge mockada; teste de contrato de tipos; smoke manual Windows.

**Esforço:** 3–4 semanas.

---

### F11 — E2E Windows, caos, release assinado, updater e desinstalação

**Objetivo:** provar o fluxo completo e fechar a entrega.

**Depende de:** F0–F10.

**Entregáveis:**

| Caminho | Ação | Responsabilidade |
|---|---|---|
| `src/tools/fake-litellm/` | criar/expandir | Fake de health, messages/SSE, credentials, errors |
| `src/tools/fake-provider/` | criar | Provider OpenAI-compatible fake, se necessário |
| `src/ui/e2e/` | criar | Specs WebdriverIO/tauri-driver |
| `scripts/e2e/run.ps1` | criar | Build de teste, servidor de artefatos local, execução |
| `.github/workflows/e2e-windows.yml` | criar | E2E noturno/manual com fake artifacts |
| `.github/workflows/release.yml` | alterar | Assinatura do instalador, updater, SBOM, checksums |
| `src/src-tauri/src/diagnostics.rs` | criar | Relatório redigido, export |
| `src/src-tauri/src/commands/diagnostics.rs` | criar | Comandos |
| `src/src-tauri/tauri.conf.json` | alterar | Bundle NSIS, updater, assinatura |
| `src/src-tauri/nsis/*` | criar | Hook de desinstalação/cleanup |
| `THIRD_PARTY_NOTICES.md` | alterar | LiteLLM/Postgres/Python/deps |
| `licenses/` | alterar | Licenças dos artefatos |
| `docs/onboarding.md` | alterar | Schema/retomada/offline implementados |
| `docs/desktop-app.md` | alterar | Módulos reais |
| `docs/gateway-architecture.md` | alterar | Proxy/CA/fluxo final |
| `docs/dev-guide.md` | alterar | Novos comandos/testes |
| `docs/adr/0001-proxy-stack.md` e afins | criar | Decisões estruturais |

**Fora de escopo:** novos recursos de produto; multi-perfil; i18n.

**Critérios de aceite:**

- E2E em Windows: instalação limpa → wizard API key → integração Figma → teste; Copilot com fake; offline; skip; reconfiguração; upgrade; uninstall.
- Caos: kill de sidecar, queda de rede no download, disco cheio, porta ocupada, proxy preexistente, token expirado, config corrompida.
- Regressão não-IA: 0 falhas.
- Instalador 15–30 MB; runtime baixado.
- Instalador e binários assinados; updater verifica assinatura.
- Desinstalação remove env/CA/segredos; pergunta sobre dados.
- `pnpm build`, `pnpm test`, clippy, testes, cobertura e `scripts/check_docs.py` verdes.
- Primeiro run ≤10 min em rede típica.

**Testes:** E2E, caos, regressão, contrato, instalação/desinstalação, updater; smoke manual com provider real.

**Esforço:** 3–4 semanas.

---

## 8. Estratégia de testes por fase

### 8.1 Regras gerais

- Nenhum teste unitário/integração usa rede externa; fake servers locais.
- Fixtures sem segredos/prompts reais; capturas reais redigidas.
- Todo bug corrigido ganha teste de regressão.
- `ffg-core` continua com gate **≥80% de linhas**; não usar `#[cfg(not(test))]` ou ignores para maquiar cobertura.
- Novos crates: meta separada de cobertura (ex.: `ffg-runtime` e `ffg-proxy` ≥70%), sem substituir o gate do core.
- PR sem teste para comportamento alterado não passa.
- Testes Windows de plataforma no `windows-latest`; lógica pura no Ubuntu.

### 8.2 Matriz por fase

| Fase | Unitário | Integração | Contrato | E2E | Caos | Regressão |
|---|---|---|---|---|---|---|
| F0 | Setup/erros/serialização | Store + .bak | TS/Rust campos | — | Corrupção | — |
| F1 | Manifesto/config/env/journal/copilot | — | Golden YAML/assinatura | — | Property tests | Rewrite host/path |
| F2 | Scripts/verificador | Smoke Windows | Manifesto real | — | 1 byte alterado | — |
| F3 | Planos/hash/zip | Fake HTTP + resume | Artefato F2 | Instalação manual | Disco/rede/zip | — |
| F4 | Backoff/portas/estado | Fake child/crash | Health LiteLLM | Start/stop | Kill externo | — |
| F5 | Config/provider validation | Fake LiteLLM | `/v1/messages` | Provider real manual | 401/404/429 | Config anterior |
| F6 | CA/leaf/thumbprint | WinCrypt | Chain TLS | Certutil | Política negada | — |
| F7 | Route/headers/stream | Fake TLS upstream | SSE/count_tokens | Curl/proxy | Timeout/cancel/h2 | Não-IA |
| F8 | Env plan/journal | Registry/CA/proxy | TLS/rewrite | Figma restart | Rollback parcial | Env exato |
| F9 | Copilot states | Postgres real/fake API | Credential/device flow | Copilot manual/fake | 401/plano/timeout | Perfil leve |
| F10 | Componentes/store | Bridge mock | Tipos | Wizard Windows | Erros/retry | Dashboard |
| F11 | Diagnóstico | E2E harness | — | Completo | Todos | 0 não-IA |

### 8.3 Mínimo para o `ffg-core` (gate 80%)

Testar obrigatoriamente:

- `setup`: todas as transições, skip, stale, resume, reset por escopo, migração, corrupção.
- `manifest`: parse, assinatura válida/inválida/chave desconhecida/expirada, bytes alterados, `minAppVersion`, política de versão, hash.
- `download`: 206/200/416, ETag/If-Range, retry/mirror, meta de resume, matemática de disco.
- `install`: path traversal, symlink, zip bomb, layout, versão, ponteiro ativo, rollback.
- `litellm_config`: golden por provider, fallback, rotas inválidas, nomes públicos, YAML parseável, ausência de segredo.
- `env_plan`: merge/restore, `NO_PROXY`, valores ausentes, userinfo redigido.
- `integration`: ordem de passos, rollback inverso, idempotência, status partial.
- `copilot`: estados, timeout, 401/plano, disconnect.
- `redaction`: headers/URL/body/segredos com fixtures.
- `checks`: plataforma, disco, internet, Figma, proxy preexistente, admin.
- `providers`: campos obrigatórios, Azure, local, nomes `claude-*`/`anth-*`.
- `models`: fallback sem ciclo, aliases duplicados, modelo público inválido.
- `rewrite`: prefixos e query (já existente; ampliar para variantes).

### 8.4 Regressão de tráfego não-IA

Matriz mínima no `ffg-proxy`:

| Host/path | Esperado |
|---|---|
| `www.figma.com`, `mcp.figma.com`, `desktop.figma.com` | CONNECT/túnel raw, sem MITM |
| `localhost`, `127.0.0.1`, `::1` | túnel direto |
| `api.figma.com/api/files/...` | MITM + forward intacto; auth original preservado |
| `api.figma.com/ai-gateway/.../v1/messages` | rewrite + master key + LiteLLM |
| `api.figma.com/ai-gateway/.../v1/messages/count_tokens?beta=true` | rewrite + query preservada |
| `api.figma.com` upgrade/WebSocket | passthrough/upgrade suportado |
| Host desconhecido | túnel |

Assertivas: nenhum `LITELLM_MASTER_KEY` no upstream não-IA; nenhum token do Figma no LiteLLM; corpo/headers hop-by-hop preservados; SSE sem buffering; 4 streams; cancelamento.

### 8.5 CI

Atualizar `.github/workflows/ci.yml`:

- `ci-rust`: fmt, clippy `-p ffg-core -p ffg-runtime -p ffg-proxy --all-targets -- -D warnings`, testes.
- `ci-coverage`: mantém `ffg-core --fail-under-lines 80`; job separado para runtime/proxy ≥70%.
- `ci-web`: `pnpm test` + `pnpm build`.
- `ci-tauri`: Windows, clippy do app, `pnpm tauri build --no-bundle`, testes de plataforma.
- `ci-docs`: `scripts/check_docs.py`.
- `ci-e2e-windows`: noturno/manual com fake artifacts, `tauri-driver`.
- `ci-artifacts`: release workflow F2.

---

## 9. Riscos e mitigação

| Risco | Prob. | Impacto | Mitigação | Verificação |
|---|---|---|---|---|
| Chave Ed25519 comprometida/rotação | Média | Alto | Chave em secret protegido; `keyId`; chaves atual+próxima embutidas; release assinado; plano de rotação | Teste de chave desconhecida/expirada; auditoria do workflow |
| PyInstaller/antivírus/SmartScreen no runtime Python | Alta | Alto | Preferir one-folder; evitar UPX; assinar binários; testar Defender; fallback venv selado; build limpo | Smoke Windows; scan; instalação em VM limpa |
| Proxy TLS/HTTP2/SSE/Upgrade incorreto | Alta | Alto | Spike hudsucker vs hyper; modo passthrough; testes de contrato; fail-closed; logs claros | Testes de streaming/h2/upgrade; E2E Figma |
| CA bloqueada por política corporativa | Média | Alto | `CurrentUser\Root`; detecção de erro; exportação manual; fallback `NODE_EXTRA_CA_CERTS` documentado | Teste em máquina com política; validação TLS |
| Loop de proxy (LiteLLM/downloader usando nosso proxy) | Média | Alto | `NetworkContext` separa proxy externo original; limpar `HTTP(S)_PROXY` dos filhos; `NO_PROXY`; teste de loop | Teste com env preexistente; inspeção de processo |
| Proxy preexistente sobrescrito | Média | Alto | Snapshot criptografado; confirmação; restauração exata; sem encadeamento no MVP | Teste de restore; regressão env |
| Mudança de host/path/versão do Figma | Média | Alto | Hosts/path configuráveis; detecção de versão; modo passthrough; contrato capturado | Fixtures; teste com versão de referência |
| Copilot sem API estável/ToS/plano | Alta | Alto | Spike obrigatório; fallback UI; erros claros; revisão legal; device flow com timeout | Teste com conta real; matriz 401/403/404/429 |
| Postgres corrompido/migração | Média | Alto | Init controlado; backup antes de update; readiness; reparo/reinit com confirmação; major fixo | Teste de corrupção/upgrade; backup/restore |
| Porta ocupada | Alta | Médio | Alocação dinâmica; persistência; revalidação; atualizar env e avisar | Teste de conflito; restart |
| UAC/admin/política | Média | Alto | Tudo por usuário; detectar elevação; instruções manuais; sem serviço Windows | Teste em conta padrão; política |
| WebView2 ausente/antigo | Média | Alto | Bootstrapper no instalador; detecção; mensagem clara | Instalação em Windows 10 limpo |
| CI Windows lento/flaky | Alta | Médio | Cache; testes fake; E2E noturno; timeouts; separar jobs | Histórico de CI |
| Download grande/disco cheio | Alta | Médio | Preflight; resume; espelhos; limpeza de versões antigas; progresso | Teste de disco; download real |
| Vazamento de segredos em logs/eventos | Média | Alto | Redaction central; revisão de logs; testes com fixtures; sem corpo/prompt | Teste de redaction; grep no CI |
| Env órfão após crash | Média | Alto | Startup repair; journal; aviso; remoção no quit; diagnóstico | Teste de crash simulado |
| Abuso do proxy local (injeção de master key) | Média | Alto | Bind loopback; ACL; considerar token aleatório no proxy; documentar risco residual | Teste de acesso não-loopback; revisão de segurança |
| Estado `setup.json` corrompido | Média | Médio | Escrita atômica; `.bak`; quarentena; migração | Teste de corrupção |
| Licenças/redistribuição (LiteLLM/Postgres/Python) | Média | Alto | Revisão; notices; SBOM; excluir `enterprise/`; licença PostgreSQL | Auditoria de release |
| Startup Python lento/timeout | Alta | Médio | Health readiness com timeout maior; progresso; rollback; logs tail | Teste em máquina lenta |
| Caminhos com espaço/acentos/long path | Média | Médio | Args sem shell; `--locale=C`; testes em pt-BR; paths canônicos | Teste Windows pt-BR |

---

## 10. Dependências, ordem e PRs

### 10.1 Grafo de dependências

| Fase | Depende de | Pode paralelizar com | Marco |
|---|---|---|---|
| F0 | — | — | Contratos/estado |
| F1 | F0 | — | Núcleo puro |
| F2 | F1 | F3 (com fixtures) | Artefatos assinados |
| F3 | F1, F2 (teste real) | F2 | Runtime instalável |
| F4 | F3 | F2/F5 | LiteLLM supervisionado |
| F5 | F1, F4 | F6/F7 | Providers API key |
| F6 | F1 | F5/F9 | CA confiável |
| F7 | F1, F6 | F9 | Proxy validado |
| F8 | F4, F6, F7 | F9 | Integração Figma |
| F9 | F4, F5 | F6–F8 | Copilot conectado |
| F10 | F0, F3, F5, F8, F9 | Pode começar após F0 com mocks | Wizard completo |
| F11 | F0–F10 | — | Release |

Caminho crítico: **F0 → F1 → F2 → F3 → F4 → F5 → F9 → F10 → F11**; ramo do proxy: **F1 → F6 → F7 → F8 → F10**.

### 10.2 PRs sugeridos

Um PR = uma unidade lógica, com teste e, quando aplicável, atualização do doc afetado. Sugestão:

| # | PR | Fase | Depende | Conteúdo |
|---|---|---|---|---|
| 1 | `docs(adr): contratos do onboarding` | F0 | — | ADR do setup schema, paths, segredos e eventos |
| 2 | `feat(core): setup state machine e erros` | F0/F1 | 1 | Tipos, transições, `UiError`, testes |
| 3 | `feat(app): estado, paths e setup store` | F0 | 2 | `AppState`, store atômico, comandos base |
| 4 | `feat(ui): contratos e store tipado` | F0 | 3 | `contracts.ts`, bridge, dashboard extraído |
| 5 | `feat(core): manifesto assinado e política` | F1 | 2 | Ed25519, semver, `RuntimePolicy`, fixtures |
| 6 | `feat(core): planos de download/install` | F1 | 5 | Range/resume, zip safety, rollback |
| 7 | `feat(core): config LiteLLM e providers` | F1 | 2 | YAML, catálogo, validação, golden |
| 8 | `feat(core): env plan, journal e redaction` | F1 | 2 | Restore, rollback, logs seguros |
| 9 | `build(release): runtime LiteLLM/Postgres` | F2 | 5 | Scripts, workflow, smoke |
| 10 | `feat(release): manifesto assinado` | F2 | 9 | Signer/verifier, chaves públicas |
| 11 | `feat(runtime): download/resume` | F3 | 6 | HTTP Range, `.part`, retries |
| 12 | `feat(runtime): extração/instalação atômica` | F3 | 11 | Zip safety, probe, active pointer, rollback |
| 13 | `feat(app): comandos e eventos de runtime` | F3 | 12 | `runtime_*`, progresso, import offline |
| 14 | `feat(app): supervisor e Job Object` | F4 | 13 | Processos, health, logs, portas |
| 15 | `feat(app): config store e lifecycle LiteLLM` | F5 | 14 | Config transacional, restart, health |
| 16 | `feat(app): providers e teste de conexão` | F5 | 15 | Catálogo, secrets, teste, rollback |
| 17 | `feat(proxy): CA e leaf certs` | F6 | 5 | Geração, DPAPI, cadeia |
| 18 | `feat(app): trust store Windows` | F6 | 17 | WinCrypt install/remove |
| 19 | `feat(proxy): túnel e passthrough` | F7 | 17 | CONNECT, roteamento, forward |
| 20 | `feat(proxy): MITM, rewrite e auth` | F7 | 19 | TLS seletivo, headers, LiteLLM |
| 21 | `feat(proxy): SSE, h2 e upgrade` | F7 | 20 | Streaming, backpressure, testes |
| 22 | `feat(app): controller e validação do proxy` | F7 | 21 | Start/stop, health, first-ai |
| 23 | `feat(app): env HKCU e detecção do Figma` | F8 | 18 | Registry, broadcast, locator |
| 24 | `feat(app): integração transacional Figma` | F8 | 22,23 | Journal, install/remove, validate |
| 25 | `feat(app): Postgres embutido` | F9 | 14 | Init/start/stop/backup |
| 26 | `feat(app): Copilot credential/device flow` | F9 | 15,25 | Credential, OAuth, poll, alias |
| 27 | `feat(app): teste e troca de perfil Copilot` | F9 | 26 | Teste, erros, disable |
| 28 | `feat(ui): shell, dashboard e wizard 0–3` | F10 | 4,13,16 | Telas iniciais, store, eventos |
| 29 | `feat(ui): passos Figma e finish` | F10 | 24,28 | Integração, status, autostart |
| 30 | `feat(ui): acessibilidade e testes` | F10 | 29 | Axe, teclado, foco, responsivo |
| 31 | `test(e2e): fake sidecars e harness` | F11 | 28–30 | Fake LiteLLM/provider, tauri-driver |
| 32 | `build(release): assinatura, updater e uninstall` | F11 | 31 | NSIS, signtool, updater, SBOM |
| 33 | `docs: onboarding e troubleshooting final` | F11 | 32 | Docs/ADRs/notices |

### 10.3 Paralelização

- F2 e F3 podem andar juntos: F3 usa fake artifacts até F2 publicar.
- F5, F6 e F9 podem ser paralelos depois de F4; F7 depende de F6.
- A UI shell (F10) pode começar logo após F0 com bridge mockada; os passos reais entram conforme os backends ficam prontos.
- F11 pode começar o harness fake após F4, antes do wizard final.
- Não paralelizar a mesma área de arquivo sem coordenação: `lib.rs`, `commands/mod.rs`, `Cargo.toml` e `bridge.ts` são pontos de conflito.

### 10.4 Esforço estimado

| Fase | Pessoa-semanas | Observação |
|---|---:|---|
| F0 | 1–2 | Contratos e shell |
| F1 | 2–3 | Núcleo puro e testes |
| F2 | 2–3 | Build/assinatura/smoke |
| F3 | 2–3 | Downloader robusto |
| F4 | 2 | Supervisor/Job Object |
| F5 | 2–3 | Providers/config transacional |
| F6 | 1–2 | CA/trust store |
| F7 | 4–6 | Proxy MITM é o maior risco |
| F8 | 2–3 | Env/Figma/rollback |
| F9 | 3–4 | Copilot/Postgres |
| F10 | 3–4 | Wizard completo/a11y |
| F11 | 3–4 | E2E/release |
| **Total** | **27–39** | 1 dev sênior; 2 devs reduzem calendário |

---

## 11. Decisões em aberto

Antes de codar, confirmar (recomendação entre parênteses):

1. **Formato da assinatura do manifesto:** detached `.sig` sobre bytes exatos vs envelope JSON canônico.
   **Recomendação:** detached `.sig` com `keyId`, assinatura sobre `manifest.json` bruto.
2. **Hospedagem/espelho:** GitHub Releases + S3/CloudFront vs só GitHub.
   **Recomendação:** GitHub primário e espelho S3 configurável; confirmar credenciais/custo.
3. **API do Copilot no LiteLLM `v1.108.1`:** existe API estável para criar credential `github_copilot` e consultar status do device flow?
   **Recomendação:** spike de 2–3 dias; se não houver, usar UI do LiteLLM com polling e resolver a autenticação local sem expor master key.
4. **Mapeamento de modelos Copilot** para `claude-opus-4-8`/`claude-haiku-4-5`: quais IDs o plano do usuário aceita?
   **Recomendação:** catálogo versionado + descoberta/validação em runtime; não hardcodar nomes não confirmados.
5. **Versão/pacote do Postgres 16.x:** qual patch oficial e como redistribuir.
   **Recomendação:** binários oficiais EDB, licence PostgreSQL incluída; confirmar com revisão legal.
6. **Validade da CA e comportamento ao sair:** manter CA instalada? Root 2–3 anos, leaf 90 dias?
   **Recomendação:** root 2–3 anos, leaf 90 dias; manter CA no quit, remover env; remover CA no “Remover integração”/uninstall.
7. **Proxy preexistente:** substituir com snapshot, mesclar ou encadear?
   **Recomendação:** snapshot + substituição explícita; sem encadeamento no MVP; restaurar exatamente.
8. **Fail-open vs fail-closed:** qual padrão?
   **Recomendação:** fail-closed; fail-open apenas opcional e, se implementado, só para requests bufferizáveis (não streaming).
9. **Autostart padrão:** ligado ou desligado?
   **Recomendação:** desligado até o usuário marcar no finish.
10. **Importação offline sem manifesto assinado:** permitir?
    **Recomendação:** não por padrão; modo avançado com confirmação explícita e warning.
11. **Telemetria/diagnóstico:** enviar algo?
    **Recomendação:** nenhum envio; apenas export local redigido sob demanda.
12. **Assinatura de código:** certificado OV/EV/Azure Trusted Signing?
    **Recomendação:** assinatura obrigatória para release público; confirmar orçamento/provedor.
13. **Idioma:** pt-BR only?
    **Recomendação:** pt-BR no MVP; strings isoladas para i18n futuro.
14. **Hosts do Figma:** stable apenas ou staging/gov?
    **Recomendação:** `api.figma.com` padrão; staging/gov configuráveis, não habilitados por padrão.
15. **Dados no uninstall:** apagar runtime/Postgres/segredos?
    **Recomendação:** remover integração/segredos por padrão; perguntar sobre runtime e dados do Postgres.
16. **Armazenamento de segredos:** `keyring` vs DPAPI direto para API keys.
    **Recomendação:** Credential Manager via `keyring`; DPAPI só para chave da CA e snapshots de env.
17. **Atualização de runtime:** app-pinned vs manifesto atualizável.
    **Recomendação:** app-pinned; atualização de LiteLLM/Postgres só em release nossa com novo app.
18. **Autenticação do proxy local:** sem auth vs token aleatório no URL do proxy.
    **Recomendação:** loopback-only no MVP; avaliar token aleatório se o Figma honrar userinfo no `HTTPS_PROXY`; documentar risco residual.
19. **HTTP/2 e WebSocket/Upgrade:** obrigatório no MVP?
    **Recomendação:** HTTP/1.1 CONNECT obrigatório; h2 e Upgrade como must-have de regressão não-IA, com testes; se `hudsucker` não cobrir, tunnelar Upgrade.
20. **Fixtures reais de contrato:** precisamos de captura do Figma 126.11.7?
    **Recomendação:** sim, spike de captura redigida antes de F7; sem isso, usar fixtures sintéticas e marcar validação pendente.

---

## 12. Checklist final de aceite do onboarding

- [ ] Windows x64, sem admin, instalador 15–30 MB.
- [ ] First-run completo em ≤10 min em rede típica.
- [ ] LiteLLM `v1.108.1` baixado, verificado, extraído e validado por `--version`.
- [ ] Manifesto assinado Ed25519 + SHA-256; rollback em falha.
- [ ] Supervisor sobe/para/reinicia LiteLLM e Postgres; Job Object sem órfãos.
- [ ] Provider API key configurado e testado; config transacional.
- [ ] Copilot: Postgres + credential + OAuth/device flow + aliases + teste.
- [ ] `config.yaml` sem segredos; restart com rollback.
- [ ] CA em `CurrentUser\Root`; env em `HKCU\Environment`; proxy loopback.
- [ ] TLS + rewrite validados; primeira requisição de IA marca verde.
- [ ] Tráfego não-IA intacto (0 regressões).
- [ ] Remoção restaura env exatamente e remove só a CA nossa.
- [ ] Fechar/reabrir retoma; tray “Reconfigurar” funciona.
- [ ] Modo offline “Já tenho os pacotes” funciona com manifesto assinado.
- [ ] Pular Figma permitido com badge pendente.
- [ ] Teste ponta a ponta e diagnóstico redigido.
- [ ] `cargo fmt`, clippy `-D warnings`, testes, cobertura ≥80% `ffg-core`, `pnpm build`, `pnpm test`, E2E Windows e `check_docs.py` verdes.
- [ ] Nenhum segredo em diff, logs, eventos ou arquivos de estado.
- [ ] Licenças/notices atualizados; sem `enterprise/` do LiteLLM.

> Este documento é o plano de implementação; a implementação ainda não foi iniciada.

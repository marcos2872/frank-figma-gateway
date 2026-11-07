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
- [x] `setup_get_state` reconcilia o runtime instalado (`active.json`) com o `setup.json`, para o
  wizard não pedir download de novo depois de uma instalação concluída.
- [x] `ui`: contratos TypeScript, bridge tipada, eventos e store do dashboard.
- [x] `ui`: dashboard extraído de `App.tsx` e passando a consumir o store.
- [ ] Troca atômica real no Windows com `ReplaceFileW` (o fallback atual remove + renomeia).
- [x] Compilação e build Tauri validados no Windows/MSVC (`clippy`, testes e `tauri build --no-bundle`).
- [ ] Execução real de registry, keyring, DPAPI e trust store no Windows.
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

- [x] Workflow `.github/workflows/release-runtime.yml` para tags `runtime-v*`: build Windows,
  manifesto, assinatura, verificação, release versionado e canal `runtime-latest`.
- [x] `scripts/build-litellm-runtime.ps1` (PyInstaller one-folder + validação `--version`).
- [x] `scripts/build-pgsql-runtime.ps1` (binários oficiais do EDB + `postgres --version`).
- [x] `scripts/manifest.mjs` (SHA-256, tamanho, URL, `installedSize`, entrypoint).
- [x] Entrypoint do runtime força UTF-8 no stdio: o banner do LiteLLM usava box-drawing e o
  binário congelado abria o pipe como cp1252, derrubando o proxy com `UnicodeEncodeError`.
- [x] Ferramentas `manifest-signer` e `manifest-verify` (Ed25519, `keyId`, janela de validade).
- [x] Recursos embutidos: `resources/runtime-policy.json` e `resources/keys/manifest-keys.json`
  (placeholder `retired`).
- [x] `docs/release.md` e entrada no índice da documentação.
- [x] Chave pública real `2026-01` embutida em `resources/keys/manifest-keys.json`
  (placeholder `retired` removido).
- [x] `scripts/generate-manifest-key.mjs` gera o par fora do repositório; `scripts/check-manifest-key.mjs`
  roda no job de assinatura e falha se a chave do app divergir dos secrets.
- [x] Release notes geradas pelo workflow (conteúdo, verificação e requisitos) e canal estável
  `runtime-latest` com upload do manifesto por último.
- [ ] Validar o build real do LiteLLM/Postgres e o workflow no Windows.

### F3 — Downloader e instalação atômica

- [x] Novo crate `ffg-runtime` com downloader HTTP: `Range`/`If-Range`, retomada por `.part` +
  `.meta.json`, rotação de espelhos, retry e verificação SHA-256/tamanho.
- [x] Extração ZIP segura com validação de path/symlink/duplicata/zip bomb e entrypoint.
- [x] Teto de entradas do ZIP ajustado para 50 mil (o pacote do Postgres do EDB tem ~22,6 mil:
  locale, timezone, include); a proteção contra zip bomb segue no tamanho descomprimido e na razão.
- [x] Instalação atômica em `runtime/<componente>/<versão>`, ponteiro `active.json` e rollback.
- [x] Probe de `--version` com timeout.
- [x] Fetch e verificação de manifesto assinado (online e offline).
- [x] `RuntimeManager` como fachada de manifesto/download/instalação.
- [x] Comandos Tauri `runtime_status`, `runtime_fetch_manifest`, `runtime_install` e
  `runtime_cancel`, com evento `download://progress`.
- [x] `AppState` embute as chaves públicas e a política de runtime.
- [x] Testes com `wiremock` para download/resume/hash, extração, instalação, manifesto e offline.
- [ ] E2E real de download/instalação no Windows (artefatos de F2, paths longos/acentos).
- [x] Crate restaurado no repositório: `src/.gitignore` ignorava `crates/runtime` com o padrão
  `runtime/`; corrigido para `/runtime/` e os 34 testes do crate passam no Windows/MSVC.
- [ ] Ligar os comandos ao wizard/dashboard (F10).

### F4 — Supervisor de sidecars

- [x] Novo crate `ffg-supervisor`: spawn/stop/restart, estado desejado, health TCP/HTTP,
  backoff exponencial, limite de restarts e monitor periódico.
- [x] Logs por serviço em ring buffer com redaction de bearer tokens e limite de linha.
- [x] Alocação de portas loopback e persistência em `setup.json`.
- [x] Job Object do Windows com `KILL_ON_JOB_CLOSE` (compilação cruzada validada; runtime pendente).
- [x] Helper de teste `spec` restrito a `unix` (no Windows virava dead code e quebrava o
  `clippy -D warnings`).
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
- [x] Mapeamento de modelo no corpo JSON: `model_aliases` (id recebido → rota) e `default_model`
  para ids desconhecidos, aplicado só em path de IA.
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
- [x] Proxy do sistema (WinINET `Internet Settings`) com snapshot/restore: o renderer da Figma é
  Chromium e ignora `HTTPS_PROXY`; sem isso só o agente Node passaria pelo MITM.
- [x] Startup reaplica o WinINET quando o journal está `active`, inclusive em instalações antigas
  (cria o snapshot se faltar); o “Instalar integração” ficou idempotente e a saída restaura o
  proxy para não deixar a máquina sem internet.
- [x] Startup sobe os sidecars quando a integração está ativa: sem o LiteLLM o proxy respondia 502
  depois de reiniciar o app.
- [x] LiteLLM passou a escutar só em `127.0.0.1` (antes `0.0.0.0`).
- [x] Instalação transacional: CA → proxy → env → validação, com journal em
  `state/integration.json`.
- [x] Snapshot/restore do env protegido por DPAPI (fallback plain nos testes).
- [x] Detecção do Figma (`Figma.exe`, versão `app-*`, processo) e restart controlado.
- [x] Comandos `figma_environment`, `figma_install`, `figma_validate`, `figma_remove`,
  `integration_status`, `figma_restart` e wrappers `install/remove_figma_integration`.
- [x] Validação: CA instalada, proxy rodando, HTTPS_PROXY/HTTP_PROXY/NO_PROXY, self-test de
  TLS/rewrite e presença do Figma.
- [x] Tray “Reconfigurar”/“Configurar Figma” emite `wizard://open`; a UI abre o modal.
- [x] Validado no Windows: instalação via startup, restart do Figma e renderer conectado ao proxy
  do sistema (27 conexões em `127.0.0.1:57995`).
- [x] Validado no Windows: proxy → rewrite → LiteLLM → OpenRouter com mensagem, `count_tokens` e
  streaming SSE (`200` e chunks reais).
- [ ] Validar remoção completa (CA + env + WinINET) com o Figma real.
- [ ] Tratar rollback/erro parcial com o app em execução e trabalho não salvo.

### F9 — Postgres embutido e Copilot

- [x] Novo crate `ffg-postgres`: layout, `initdb`, `psql`/CREATE DATABASE, servidor loopback e
  `DATABASE_URL` com senha URL-encoded.
- [x] Comandos `copilot_status`, `copilot_prepare`, `copilot_disable` e `copilot_disconnect`.
- [x] `copilot_prepare` inicializa o cluster, sobe o Postgres, cria o banco `litellm`, persiste a
  senha no cofre e marca `postgresReady`.
- [x] `DATABASE_URL` injetado no ambiente do LiteLLM quando o perfil Copilot está ativo.
- [x] Senha do Postgres protegida pelo secret store; dados em `postgres/data`.
- [x] Device flow do GitHub Copilot em `ffg-litellm::copilot`: `device/code`, polling
  (`authorization_pending`/`slow_down`/`expired_token`/`access_denied`), login e gravação do
  `access-token` em `copilot/github-copilot/`, injetado como `GITHUB_COPILOT_TOKEN_DIR` no LiteLLM.
- [x] `runtime_install` sincroniza `setup.runtime` (inclusive o hash do ZIP).
- [x] Copilot sem Postgres: o token do device flow é a credencial. `DATABASE_URL` não é mais
  injetado (o runtime empacotado não inclui o Prisma) e as chaves de provider vão para o env do
  LiteLLM a partir do cofre.
- [x] Rota `github_copilot/` não exige mais `litellm_credential_name` (autentica pelo token).
- [ ] Testar o fluxo completo com uma conta Copilot real e confirmar o modelo do plano.
- [ ] Revisão de ToS do OAuth (reusa o client id público do LiteLLM) antes do release público.
- [ ] Backup/restore, upgrade de major e validação do cluster no Windows.

### F10 — Wizard completo

- [x] Modal + carrossel ligado aos passos reais: boas-vindas, runtime, provedor, modelos, Figma e conclusão.
- [x] Passo 0 com `environment_report` (plataforma, data dir, disco, internet, Figma e proxy preexistente).
- [x] Passo 1 com download/instalação do LiteLLM, progresso via `download://progress` e cancelamento.
- [x] Passo 2 com catálogo de provedores, formulário dinâmico, salvar credencial e preparar Postgres do Copilot.
- [x] Passo 3 com aliases dos ids que o cliente envia (codenames da Figma + `claude-*`), fallback, aplicação do config, start dos serviços e teste real no LiteLLM.
- [x] Passo 3 lista os ids do seletor do Make (`FIGMA_MODEL_IDS`) com uma rota por id; o mapa vai em
  `models.aliases` e o proxy o aplica.
- [x] Passo 4 com instalar/validar/remover/reiniciar Figma e checks da integração.
- [x] Passo 5 com status dos serviços, saúde do LiteLLM, integração Figma, autostart e conclusão.
- [x] First-run abre o wizard automaticamente; tray “Reconfigurar”/“Configurar Figma” abre o modal.
- [ ] Importação offline (“Já tenho os pacotes”) ainda não tem comando/UI de file picker.
- [ ] Testes de UI (Vitest/RTL/axe), teclado e leitor de tela pendentes.
- [ ] E2E do wizard completo no Windows com artefatos reais.

### Correções de UI — modal do wizard

- [x] Rolagem: o `.wizard-viewport` passou a ser o container de rolagem (`overflow-y: auto`); antes
  o `overflow-y` ficava no slide, que não tinha altura definida, e o conteúdo era cortado.
- [x] Scroll volta ao topo ao trocar de etapa.
- [x] Mensagens de erro: o `UiError` devolvido pelo `invoke` é normalizado em `ui/src/lib/errors.ts`;
  antes a UI exibia `[object Object]`.
- [ ] Download real do LiteLLM continua bloqueado: não há release publicado (manifesto responde 404)
  e a chave embutida é o placeholder `retired`.

### Modelos — validação na 126.11.7

- [x] Ids reais mapeados no bundle do web app: `default` (Auto), `anthropic-claude-*`,
  `figaro-gpt-*`, `aqua` (Gemini 4 Argon), `google-gemini-*`, `velvet`/`goldenrod`/`emerald`/`tangelo`.
- [x] A lista do seletor vem de `get_supported_models` (**IPC** com o agente local), não de HTTP;
  o nome exibido é um `switch` fixo no bundle.
- [x] Consequência: o proxy substitui o modelo no corpo de `/v1/messages`, mas **não** injeta nomes
  na lista do seletor.
- [x] Mapa configurável id → rota no proxy (`model_aliases` + `default_model`) e validação de nome
  público sem exigir prefixo `claude-`/`anth-`.
- [x] UI do passo 3 para escolher a rota de cada id do Make (`models.aliases`).
- [ ] Confirmar em execução qual id chega em cada fluxo (Make, agente, tarefas auxiliares).

### Release de runtime — iteração

- [x] Release do runtime separado do app: `release-runtime.yml` em tags `runtime-v*` ou
  `workflow_dispatch`, release versionado + canal `runtime-latest` (o app lê
  `releases/download/runtime-latest/manifest.json`).
- [x] `DEFAULT_MANIFEST_URL` aponta para o canal estável; releases do app (`v*`) não o afetam.
- [x] Chave `2026-01` gerada e embutida; assinatura e verificação testadas localmente com
  `manifest-signer`/`manifest-verify`.
- [x] Release notes com conteúdo, verificação por SHA-256 e requisitos.
- [x] `publish` ganhou `actions/checkout` (o `gh` precisa do repositório para resolver a tag).

Runs de validação — nenhum release publicado ainda:

| Tag | Resultado |
|---|---|
| `runtime-v1.108.1-r1` | falhou no build: `ImportError: cannot import name 'run_server' from 'litellm.proxy.proxy_server'` |
| `runtime-v1.108.1-r2` | falhou no build: `SyntaxError` no `entrypoint.py` gerado pelo here-string do PowerShell |
| `runtime-v1.108.1-r3` | falhou; `r4` estava em andamento no fechamento deste registro |

- [ ] Publicar o primeiro release e confirmar o download real no app.

### Validação no Windows — pendências

> Lista consolidada de tudo que ainda precisa ser executado/validado em uma máquina Windows antes de
> considerar o onboarding pronto para release.

**Toolchain e build**
- [x] `cargo clippy -p frank-figma-gateway --all-targets -- -D warnings` com MSVC.
- [x] `pnpm tauri build --no-bundle` (binário gerado em `src/target/release/frank-figma-gateway.exe`).
- [ ] Build do instalador NSIS/MSI e assinatura.
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
- [ ] Workflow `release-runtime.yml` em runner Windows; publicação de ZIPs + `manifest.json` + `.sig`.
- [x] Chave pública real `2026-01` embutida no app (placeholder `retired` removido).
- [ ] Garantir `MANIFEST_SIGNING_KEY`/`MANIFEST_KEY_ID`/`MANIFEST_PUBLIC_KEY` no GitHub antes da tag.
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
- [x] OAuth/device flow real implementado (token em `GITHUB_COPILOT_TOKEN_DIR`).
- [ ] Teste com conta real, modelo do plano e erros 401/403/429.
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

### Evidências Windows (x64, MSVC)

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings` (app Tauri incluído).
- `cargo test --workspace` — **195 testes, 0 falhas** (core 117, litellm 12, runtime 34,
  supervisor 10, src-tauri 10, proxy 8, postgres 4).
- `cargo test -p ffg-proxy` — inclui o MITM com `figaro-gpt-6-sol` chegando ao upstream como
  `claude-opus-4-8`.
- `pnpm build`
- `pnpm tauri build --no-bundle` — binário em `src/target/release/frank-figma-gateway.exe`.
- `scripts/check_docs.py` (equivalente em Node; não há Python na máquina) — 13 markdown, 4 json.

> `cargo llvm-cov` não está instalado nesta máquina; o gate de cobertura segue apenas na CI.
> Ainda não executado em runtime: keyring/DPAPI, trust store, `HKCU\Environment`, `initdb`,
> download real dos artefatos e Figma real.

### Limitação descoberta — IA cloud vs Make Local (2026-11-06)

**O que foi validado**
- [x] Proxy + CA + rewrite + LiteLLM: `POST /ai-gateway/.../v1/messages` pelo proxy → `200`,
  `count_tokens` → `200` e SSE com chunks reais.
- [x] O Figma usa o proxy do sistema (WinINET) quando ele está aplicado; o app reaplica no startup
  e sobe os sidecars.

**Por que o chat cloud não passa pelo gateway**
- [x] O chat cloud (Agents e Make) usa o protocolo do agente da Figma, não uma chamada de modelo:
  - `POST /api/cortex/assistant/chat` (Agents) com `plainText`, `message.content`, `thread`, etc.
  - `POST /api/cortex/shared/figmake` (Make cloud) com `aiChatMessages` e
    `rawUserChatDetails.rawUserMessage`.
  - `GET /api/agentcontrol/subscribe` (WebSocket) com `scope_key=assistant` ou
    `chat_feature_type=figmake`, `metered_user_action=foundry` e `workload=assistant`/`make-generic`.
- [x] O servidor da Figma monta o system prompt/tools, chama o modelo e devolve os eventos pelo
  WebSocket. Não existe `/v1/messages` local; o body do cliente não é um request de modelo.
- [x] `metered_user_action=foundry` confirma que a ação é medida pela Figma; redirecionar para o
  LiteLLM seria bypass de medição, proibido pelo AGENTS.md e pelo PRD (§12).
- [x] "Trocar o destino do body" exigiria reimplementar o agente server-side (parsear o protocolo,
  montar prompt/tools, chamar o LiteLLM, traduzir eventos/tool calls e executar as ferramentas).
  Não é um proxy; é um produto novo.

**Por que o Make Local não funciona no Windows**
- [x] O único caminho client-side é o `figcode` do Make Local, que chama
  `api.figma.com/ai-gateway`.
- [x] O `figcode` é Mac-only no beta fechado: `desktop.figma.com/figcode/mac/UPDATE.json` → `200`,
  `/linux/` → `200`, `/win/UPDATE.json` → **403**.
- [x] No Windows não há processo `figcode`, nem `FigcodeData`, nem `%APPDATA%\Figma\Figcode`;
  `desktop_make_local_enabled` não está ativo e `makeLocalTrustedDirectoriesByUser` está vazio.
  O Make testado foi o cloud (`type=figmake`, `cortex/shared/figmake`), não o local.
- [x] O app 126.11.8 tem o bridge `makeLocal`/`figcode` (REA: 104 refs `figcode`, 123 `makeLocal`);
  o `app-asar` do instalador 126.11.7 em `executavel` está criptografado (REA: 0 JS parseado).

**Impacto e próximos passos**
- [x] A integração (CA + WinINET + proxy + LiteLLM) está funcional; o que não funciona é a premissa
  de substituir o modelo do **chat cloud**.
- [ ] No Windows, as opções são: plugin/companion com chat próprio usando o LiteLLM; aguardar o
  `figcode` Windows; ou usar Mac + Make Local.
- [x] A limitação está alinhada ao PRD §12 ("suportar IA fora do caminho do figcode" é
  não-objetivo), mas precisa ficar explícita no produto/onboarding.
- [x] A captura usada no diagnóstico era read-only e foi revertida; nenhum bypass foi implementado
  ou commitado.

### Próximo passo

- Configurar os secrets do release e publicar o runtime (`runtime-v1.108.1-r*`), depois confirmar o
  download no app.
- Spike obrigatório do Copilot: confirmar endpoints de credential/device flow no LiteLLM `v1.108.1`.
- Executar a checklist completa de validação no Windows acima.
- F11 — E2E, caos, release assinado, updater e desinstalação.
- Completar importação offline e testes de UI (Vitest/RTL/axe).
- PR #4 (`feat/onboarding` → `main`) aberta com todo o trabalho desta branch.

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

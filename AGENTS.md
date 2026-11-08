# AGENTS.md

Regras de trabalho neste repositório para agentes humanos e de IA. Leia antes de abrir PR, commitar ou
mexer na arquitetura.

## 1. Contexto

`frank-figma-gateway` é um app desktop **open source (MIT)** que permite ao Figma desktop usar modelos
de LLM de qualquer provedor sem alterar o app. A integração é de rede: um proxy local intercepta a
inferência do `figcode`/Claude Code e reescreve para o **LiteLLM Proxy** (sidecar, versão fixada).

Stack:

- **App:** Tauri v2 (Rust) + frontend React/Vite
- **Proxy:** Rust (MITM seletivo em `api.figma.com`)
- **Engine:** LiteLLM Proxy `v1.108.1`
- **Banco:** Postgres embutido no perfil Copilot; SQLite apenas para estado do app
- **Integração:** CA local + `HTTPS_PROXY`, sem patch no Figma

Documentação de referência: [docs/index.md](docs/index.md).

## 2. Estado atual e pendências

Hoje o repositório tem **documentação, licenças e o scaffold do app em `src/`**. CI e CD ainda não
existem.

Pendências de implementação (ordem sugerida):

- [x] Scaffold do app Tauri (tray, autostart, single-instance, close-to-tray)
- [ ] Supervisor de sidecars (LiteLLM/Postgres) com health e restart
- [ ] Proxy MITM + rewrite de path/auth + CA local
- [ ] Runtime do LiteLLM empacotado e versionado
- [ ] Postgres embutido
- [ ] Onboarding em passos (download → provider → Figma)
- [ ] Integração Copilot (OAuth + Postgres)
- [x] CI (lint, testes, build, cobertura)
- [x] Proteção da `main` (PR obrigatório + checks)
- [ ] CD (instalador assinado, manifesto assinado, updater)
- [ ] Assinatura de código e notarização
- [ ] ADRs das decisões já tomadas

## 3. Princípios

1. **Transparência** — não alterar binários, UI ou autenticação do Figma.
2. **Escopo** — implemente o pedido; não antecipe refatorações ou features.
3. **Reuso** — a camada de providers é o LiteLLM; não reimplemente adapters.
4. **Segurança primeiro** — loopback, segredos fora do código, logs redigidos.
5. **Documente decisões** — mudança de arquitetura exige atualização de docs/ADR.
6. **Licença** — nada incompatível com MIT; ver seção 13.

## 4. Estrutura planejada

```text
src/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── logging.rs
│   │   ├── supervisor.rs
│   │   ├── proxy/
│   │   ├── ca.rs
│   │   ├── env.rs
│   │   ├── postgres.rs
│   │   ├── setup.rs
│   │   ├── tray.rs
│   │   └── commands.rs
│   ├── binaries/
│   └── resources/
├── ui/
└── runtime/
docs/
├── adr/
├── figma-llm-architecture.md
├── gateway-architecture.md
├── desktop-app.md
├── onboarding.md
├── figma-gateway-integration.md
├── gateway-prd.md
└── index.md
licenses/
```

## 5. Fluxo de trabalho

- **Análise** (opinar, revisar, diagnosticar): entregue achados e plano; não edite nem rode comandos que
  alterem estado.
- **Execução** (pedido explícito): implemente só o escopo pedido, com evidência (teste/lint).
- **Ambíguo**: pergunte antes de agir.
- Leia o doc relevante antes de mudar comportamento; para ambiente e comandos, comece pelo
  [guia de desenvolvimento](docs/dev-guide.md).
- Mudou uma decisão? Atualize o doc e, se for estrutural, crie um ADR em `docs/adr/`.
- Não invente fatos sobre o Figma, o LiteLLM ou versões: verifique em `docs/` ou na fonte.

## 6. Qualidade de código

### Geral

- Código e identificadores em **inglês**; docs, commits e UI em **pt-BR**.
- Funções pequenas, responsabilidade única, erros explícitos.
- Sem segredos, tokens, prompts ou dados pessoais em logs.
- Sem `TODO` órfão: abra issue ou registre em docs.
- Dependência nova exige justificativa e checagem de licença.

### Rust

- `cargo fmt` e `cargo clippy --all-targets --all-features -- -D warnings`.
- `unwrap`/`expect` proibidos em caminho de runtime; use `Result` + `thiserror`/`anyhow`.
- `unsafe` só com justificativa, encapsulado e documentado.
- `tracing` para logs estruturados; redaction de `Authorization`, tokens e headers sensíveis.
- Nada de bloquear o runtime async; operações de processo com timeout.
- Processos filhos sempre em Job Object (`KILL_ON_JOB_CLOSE`).

### Frontend (quando existir)

- TypeScript `strict`, sem `any` gratuito.
- ESLint + Prettier; sem promessas flutuantes.
- UI não acessa segredos nem executa comandos privilegiados; tudo via comandos Tauri validados.
- Acessibilidade básica (foco, teclado, contraste).

## 7. TDD e testes

Ciclo: **red → green → refactor**. Teste primeiro para comportamento novo ou correção de bug.

| Nível | O que cobre |
|---|---|
| Unitário | Rewrite de path/auth, verificação de hash/assinatura, mapa de modelos, redaction |
| Integração | Proxy + LiteLLM fake; SSE, tools, erros, timeouts |
| Contrato | Fixtures reais de `/v1/messages` e `count_tokens` (redigidas) |
| E2E | Figma + app + provider sandbox |
| Caos | Provedor lento/erro, queda no meio do stream, token expirado |
| Regressão | Tráfego não-IA intacto; rollback de CA/env; resume do onboarding |

Regras:

- Testes determinísticos; sem rede real em unit/integração (use fake server).
- Fixtures sem segredos e sem dados pessoais.
- Todo bug corrigido ganha teste de regressão.
- Meta: ≥ 80% no núcleo (proxy, supervisor, onboarding); não perseguir 100%.
- PR sem teste para comportamento alterado não passa.

## 8. Commits

Formato **Conventional Commits**, resumo e corpo em **português**, comportamento no presente.

```text
<type>(<scope>): <resumo>

<corpo: o que estava errado/faltando e por que a mudança resolve>

<footer opcional>
```

Tipos: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `ci`, `perf`, `build`, `style`, `revert`.

Escopos sugeridos: `app`, `proxy`, `litellm`, `postgres`, `copilot`, `onboarding`, `docs`, `licenses`,
`ci`, `release`, `security`, `tests`.

Exemplos:

```text
feat(proxy): reescreve /v1/messages para o LiteLLM
fix(onboarding): retoma o download após falha de rede
test(proxy): cobre túnel para rotas não-IA
ci: adiciona clippy e testes no PR
docs(adr): registra LiteLLM como engine
```

Regras:

- Um commit = uma mudança lógica; evite misturar refactor com feature.
- Sem ponto final no resumo; ≤ 100 caracteres.
- Corpo explica o **porquê**, não o como.
- Nunca commitar `.env`, chaves, tokens ou binários gerados.
- `BREAKING CHANGE:` para mudança incompatível.
- Nunca `git push --force` em branch compartilhada; `--force-with-lease` só com autorização.
- Antes de commitar: rodar fmt/lint/testes relevantes.

## 9. Branches e PRs

- `main` é estável e **protegida**: push direto bloqueado; toda mudança entra por PR.
- Trabalho em `feat/*`, `fix/*`, `docs/*`, `chore/*`.
- PR pequeno, com descrição, evidência de teste e links para docs/issue.
- Revisão obrigatória antes de merge (quando houver mais de um mantenedor).
- Merge só com todos os checks da CI verdes, incluindo o gate de cobertura.
- Squash ou merge commit conforme o repositório definir; manter histórico legível.

## 10. CI

Implementada em [`.github/workflows/ci.yml`](.github/workflows/ci.yml). Roda em **todas as PRs**
(qualquer branch de destino) e em push para `main`.

| Job | O que roda |
|---|---|
| `ci-rust` | `cargo fmt --check`, `cargo clippy -p ffg-core -D warnings`, `cargo test -p ffg-core` |
| `ci-coverage` | `cargo llvm-cov -p ffg-core --fail-under-lines 80` + artefato `lcov.info` |
| `ci-web` | `pnpm install --frozen-lockfile`, `pnpm build` |
| `ci-tauri` | `cargo clippy -p frank-figma-gateway -D warnings`, `pnpm tauri build --no-bundle` (Windows) |
| `ci-docs` | `python3 scripts/check_docs.py` (links, fences e JSON) |

Regras:

- **Gate de cobertura**: falha se as linhas do núcleo (`ffg-core`) ficarem abaixo de **80%**.
- **Warnings são erros** no crate do app (`cargo clippy -D warnings`), inclusive `dead_code`.
- Todos os jobs são **required checks** na `main`.
- Cache de Cargo e pnpm; concorrência cancela runs antigos do mesmo ref.
- Windows é o alvo primário para o app Tauri.
- Push direto na `main` é bloqueado; toda mudança entra por PR.

## 11. CD — app pendente; runtime implementado

O **runtime** (LiteLLM/Postgres + manifesto assinado) tem workflow próprio em
[`release-runtime.yml`](.github/workflows/release-runtime.yml), disparado por tags `runtime-v*` ou
`workflow_dispatch`. Ele publica o release versionado e atualiza o canal `runtime-latest`, que é a
URL consumida pelo app.

O **app** continua pendente, a implementar em `.github/workflows/release-app.yml` (tag `v*`):

1. Build do instalador Tauri (assinado e com timestamp).
2. Smoke test em runner Windows (provider fake).
3. Publicar release com o instalador.
4. Atualizar o endpoint do updater.

Regras:

- LiteLLM e Postgres têm versão **fixada**; atualização é release nossa, nunca auto-update silencioso.
- Release com rollback: manter a versão anterior disponível.
- Não publicar sem artefatos verificados por hash.
- Assinatura de código do instalador é obrigatória para release público.
- Runtime e app usam tags separadas (`runtime-v*` e `v*`) para não disputarem o canal `latest`.

## 12. Documentação

- Entrada: [docs/index.md](docs/index.md).
- Mudou comportamento/arquitetura? Atualize o doc correspondente no mesmo PR.
- Decisão estrutural → ADR em `docs/adr/NNNN-titulo.md` (contexto, decisão, consequências).
- Diagramas em Mermaid; labels com `<br/>`, nunca `\n` literal.
- Não criar documentação sem pedido explícito, exceto atualização de doc afetado pela mudança.

## 13. Licenciamento e dependências

- Projeto sob **MIT** ([LICENSE](LICENSE)).
- Dependências permitidas: MIT, Apache-2.0, BSD, ISC, PostgreSQL License.
- GPL/AGPL só com aprovação explícita.
- Não usar código do diretório `enterprise/` do LiteLLM.
- Dependência nova → atualizar [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) e `licenses/`.
- Não redistribuir Figma desktop nem Claude Code.

## 14. Segurança e privacidade

- Bind apenas em `127.0.0.1`; portas dinâmicas.
- `LITELLM_MASTER_KEY` e `LITELLM_SALT_KEY` no Credential Manager/DPAPI.
- CA privada protegida; validade curta; remoção no uninstall.
- Logs com tokens, prompts e headers sensíveis redigidos.
- Verificar SHA-256 e assinatura dos artefatos baixados.
- Sem admin; sem inbound firewall; sem telemetria externa.
- Não contornar autenticação/licenciamento; uso exige conta Figma com IA e provider válido.

## 15. Definition of Done

Uma tarefa só está concluída quando:

- [ ] Comportamento implementado no escopo pedido.
- [ ] Testes (unit/integração quando aplicável) passando.
- [ ] `fmt`, `clippy`, `lint` e `typecheck` verdes.
- [ ] Docs/ADR atualizados.
- [ ] Licenças/notices atualizados se houve dependência nova.
- [ ] Sem segredos ou binários no diff.
- [ ] Evidência apresentada (comando + resultado).
- [ ] PR revisado (quando aplicável).

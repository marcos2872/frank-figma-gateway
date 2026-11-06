# PRD — frank-figma-gateway

> Gateway local + app desktop que permite ao Figma usar os modelos de LLM que o usuário escolher,
> sem alterar o app. Engine: LiteLLM Proxy fixado em `v1.108.1`, com app Tauri e instalador mínimo.

## 1. Contexto

O Figma desktop 126.11.7 executa a IA (Make Local) via `figcode`, que embute o Claude Code e chama
`https://api.figma.com/ai-gateway/llm/provider/universal` no formato **Anthropic Messages** com SSE.
Detalhes em [figma-llm-architecture.md](figma-llm-architecture.md).

A proposta é inserir um proxy local nessa perna, reescrevendo para um **LiteLLM Proxy** que traduz
para o provider escolhido. O app Tauri instala, configura e mantém tudo rodando no tray.

## 2. Problema

- Não existe “BYO model” no Figma desktop.
- O caminho de inferência é fechado, mas é HTTPS/SSE padrão, sem pinning e com trust configurável.
- Construir a camada de providers do zero seria caro e frágil.

## 3. Objetivos

1. Permitir que a IA do Figma use Copilot, OpenRouter, Azure AI, Anthropic, OpenAI, Gemini ou modelo
   local, sem patch no app.
2. Instalar leve e baixar componentes sob demanda.
3. Oferecer onboarding guiado, tray, autostart e close-to-tray.
4. Reusar o LiteLLM como engine de providers, com versão fixada.
5. Suportar Copilot via OAuth por usuário com Postgres embutido.
6. Ser reversível: rollback de CA/env e desinstalação limpa.

### Não-objetivos

- Contornar autenticação, licenciamento ou DRM.
- Funcionar sem conta Figma com IA habilitada.
- Cobrir IA que roda no servidor da Figma fora do `figcode`.
- Redistribuir o app da Figma ou o Claude Code.
- Alterar `app.asar` ou binários.

## 4. Personas e casos de uso

| Persona | Caso de uso |
|---|---|
| Designer/dev com IA no Figma | Usar Copilot ou modelo local no Make |
| Time com restrição de dados | Direcionar para Azure/Foundry aprovado |
| Pesquisador | Comparar modelos no mesmo fluxo |
| Usuário offline/lab | Servidor local OpenAI-compatible |

## 5. Requisitos funcionais

| ID | Requisito | Prioridade | Critério de aceite |
|---|---|---|---|
| FR-01 | Interceptar HTTPS de `api.figma.com` sem alterar binários | M | Make funciona com o proxy |
| FR-02 | Encaminhar rotas não-IA intactas | M | Nenhuma regressão em `api.figma.com` |
| FR-03 | Reescrever path e `Authorization` para o LiteLLM | M | Requisição chega ao LiteLLM com master key |
| FR-04 | Aceitar `/v1/messages` e `count_tokens` no formato Anthropic | M | Streaming, tools e erros corretos |
| FR-05 | Rodar LiteLLM como sidecar com versão fixada | M | `v1.108.1` verificada por hash |
| FR-06 | Configurar OpenRouter/Azure/Anthropic/OpenAI via API key | M | Teste de conexão OK |
| FR-07 | Suportar GitHub Copilot via OAuth + Postgres | M | “Connected” e chamada OK |
| FR-08 | Mapear `claude-opus-4-8`/`claude-haiku-4-5` para provider/modelo | M | Alias aplicado sem mudar a UI |
| FR-09 | Fallback entre modelos/providers | S | Turno continua no secundário |
| FR-10 | Instalador mínimo + download sob demanda | M | Instalador < 30 MB; runtime baixado no onboarding |
| FR-11 | Onboarding em passos com retomada | M | Fechar/reabrir retoma no passo pendente |
| FR-12 | Wizard Figma: CA + env + proxy + teste | M | Um clique instala e valida |
| FR-13 | Reiniciar Figma pelo wizard | S | Detecta processo e reabre |
| FR-14 | Tray com status e close-to-tray | M | X esconde; app continua servindo |
| FR-15 | Autostart com o sistema, oculto | M | Sobe no login e fica no tray |
| FR-16 | Single instance | S | Segunda execução foca o dashboard |
| FR-17 | Rollback da integração | M | Remove CA/env e volta ao normal |
| FR-18 | Importação offline de componentes | S | ZIP local validado por hash |
| FR-19 | Logs e health dos processos | S | Dashboard mostra estado e logs |
| FR-20 | Credenciais no Credential Manager/DPAPI | M | Nenhum segredo em texto plano |
| FR-21 | Atualização do app e dos runtimes com rollback | S | Versão anterior preservada |

## 6. Requisitos não-funcionais

| ID | Requisito | Alvo |
|---|---|---|
| NFR-01 | Instalador | ≤ 30 MB |
| NFR-02 | First-run (download + config) | ≤ 10 min em rede típica |
| NFR-03 | Latência adicional no proxy | ≤ 50 ms p95 no primeiro byte |
| NFR-04 | Streaming | Sem buffering; backpressure correto |
| NFR-05 | Concorrência | ≥ 4 streams simultâneos |
| NFR-06 | Segurança | Loopback, sem admin, sem telemetria |
| NFR-07 | Robustez | Crash de sidecar não derruba o app |
| NFR-08 | Compatibilidade | Figma 126.11.7; degradar com aviso em versões novas |

## 7. Entregáveis

### M0 — Descoberta e captura

- Proxy de captura, corpo/headers/SSE reais, confirmação do host/path.
- **Saída:** amostras redigidas + relatório.

### M1 — Casca Tauri

- Tray, close-to-tray, autostart, single-instance, dashboard vazio, logs.
- **Saída:** app instalável que roda no tray.

### M2 — Sidecar LiteLLM + download

- Manifesto assinado, download/resume/verify/extract atômico.
- Supervisor com health e restart.
- **Saída:** LiteLLM `v1.108.1` rodando sob o app.

### M3 — Proxy MITM + rewrite

- CA local, MITM seletivo, rewrite de path/auth, passthrough.
- **Saída:** Make em modo passthrough e em modo traduzido.

### M4 — Providers por API key

- OpenRouter, Azure AI, Anthropic, OpenAI, Gemini; teste de conexão.
- Mapa de modelos e fallback.
- **Saída:** e2e com provider real.

### M5 — Copilot

- Postgres embutido, credential OAuth, connect flow, refresh, erros de plano.
- **Saída:** Make usando Copilot.

### M6 — Onboarding

- Tour completo, retomada, casos de borda, importação offline.
- **Saída:** first-run de ponta a ponta.

### M7 — Empacotamento e release

- Instalador assinado, updater, desinstalador, diagnóstico.
- **Saída:** release + guia de troubleshooting.

## 8. Métricas de sucesso

| Métrica | Meta |
|---|---|
| First-run concluído sem suporte | ≥ 90% |
| Turnos sem erro de contrato | ≥ 99% |
| Tool calls traduzidas | ≥ 98% |
| Falhas de download/hash | < 1% |
| Regressões em tráfego não-IA | 0 |
| Instalação/desinstalação | < 5 min |

## 9. Riscos e mitigação

| Risco | Impacto | Mitigação |
|---|---|---|
| Postgres embutido (lifecycle/migrações) | Alto | Supervisor, backup antes de update, teste de upgrade |
| Tradução de tool calls | Alto | Fixtures reais + matriz de compatibilidade + fallback |
| Download/artefato corrompido | Médio | Hash + assinatura + rollback |
| CA/política corporativa | Médio | Diagnóstico e instalação manual documentada |
| LiteLLM supply chain | Alto | Versão fixada (`v1.108.1`), hash, nunca `v1.107.0` |
| Mudança de path no figcode | Alto | Regra configurável + detecção de versão |
| ToS de Copilot/assinaturas | Alto | Não contornar auth; aviso legal; validar uso |
| Tamanho do runtime | Médio | Download sob demanda; perfil leve sem Postgres |

## 10. Dependências

- Figma desktop com IA habilitada (`desktop_make_local_enabled`).
- Conta/plano para o provider escolhido.
- Windows 10/11 x64 + WebView2.
- Internet no first-run (ou pacote offline).

## 11. Questões em aberto

1. Onde publicar os artefatos (GitHub Releases, S3, ambos)?
2. Copilot no first-run ou em configuração avançada?
3. Permitir pular o passo do Figma?
4. Assinatura de código do instalador/app?
5. Idioma do wizard no MVP (pt-BR ou i18n)?
6. Fail-open vs. fail-closed ao perder o LiteLLM?

## 12. Fora de escopo

- Burlar limites/autenticação.
- Reescrever prompts ou ferramentas do agente.
- Suportar IA fora do caminho do `figcode`.
- Distribuir binários da Figma/Claude Code.

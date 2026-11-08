# Diagnóstico — IA cloud vs. harness local no Figma desktop

> **Data:** 2026-11-07
> **Ambiente:** Windows 11 x64; Figma desktop **126.11.8** (a `app-126.11.7` também está instalada);
> `frank-figma-gateway` em `127.0.0.1:57995`; CA e WinINET ativos.
> **Objetivo:** explicar por que o Make e a aba **Agents** continuam respondendo com o proxy parado e
> determinar se o gateway pode interceptar a inferência em algum desses fluxos.
> **Resultado:** a inferência do Make e da aba Agents é **server-side**; no Windows não existe o
> harness local (`figcode`/`claude`) que o gateway foi desenhado para interceptar.

## 1. Resumo executivo

| Pergunta | Resposta |
|---|---|
| O gateway consegue ver o tráfego do Figma? | **Sim.** Com o Figma reiniciado, o processo de rede abre conexões em `127.0.0.1:57995` (WinINET). |
| O gateway consegue trocar o modelo do Make cloud? | **Não.** A inferência acontece no servidor da Figma. |
| O gateway consegue trocar o modelo da aba Agents? | **Não.** É o mesmo caso: protocolo de controle/WebSocket, não um request de modelo local. |
| O Make Local existe nesta máquina? | **Não.** A flag `desktop_make_local_enabled` não é entregue e o Figma a remove se for injetada; `%APPDATA%\Figma\Figcode` e `FigcodeData` não existem. |
| O `figma_agent.exe` é o agente de IA? | **Não.** É um serviço Rust/actix de fontes, spellcheck, abertura de URL e updater. |
| É bug do gateway? | **Não.** O gateway, a CA e o WinINET funcionam. O alvo de inferência local não existe no Windows. |

**Conclusão:** hipótese **A3** — fluxo cloud, sem harness local. Nenhuma correção de proxy resolve;
o produto precisa decidir entre plugin/companion com chat próprio, aguardar o `figcode` Windows,
usar Mac + Make Local ou adotar um agente local externo com o LiteLLM e o Figma MCP.

## 2. Método

1. Medição de processos, conexões TCP e registro do Windows durante a operação normal e durante um
   pedido de Make.
2. Inspeção do `settings.json`, do `app.asar` (126.11.8 e 126.11.7) e do binário
   `figma_agent.exe`.
3. Teste de habilitação forçada da flag `desktop_make_local_enabled` no `settings.json`.
4. Monitoramento da aba **Agents** por 10 minutos (243 amostras) enquanto uma mensagem era enviada.
5. Comparação com o registro anterior de 2026-11-06 ([`CHANGELOG.md`](CHANGELOG.md)), que já havia
   identificado os endpoints `/api/cortex/...` e o `figcode` Mac-only.

Todas as medições foram **read-only** ou reversíveis. Nenhum bypass foi implementado.

## 3. Evidências

### 3.1 Processos e rede

Durante o Make (proxy ativo):

```text
Get-Process figcode,claude,Figma,figma_agent
→ nenhum figcode; nenhum claude
→ Figma.exe (vários processos)
→ figma_agent.exe (PID 12428)
→ frank-figma-gateway.exe (PID 4820)
```

```text
Get-NetTCPConnection -State Established
→ Figma → 127.0.0.1:57995  (21 conexões no pico)
→ brave / OpenCode / opencode-cli → 127.0.0.1:57995
→ frank-figma-gateway → :443 (túneis externos)
```

O Figma usa o proxy do sistema quando é reiniciado. Isso **descarta A2** ("o Chromium ignora o
WinINET"): o Brave e o Figma falam com `57995`. O que não existe é a chamada local de inferência.

### 3.2 WinINET e ambiente

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings
ProxyEnable   = 1
ProxyServer   = 127.0.0.1:57995
ProxyOverride = <local>;localhost;127.0.0.1;::1
AutoConfigURL = (vazio)

HKCU\Environment
HTTPS_PROXY = http://127.0.0.1:57995
HTTP_PROXY  = http://127.0.0.1:57995
NO_PROXY    = localhost,127.0.0.1,::1
```

O app grava o WinINET global. Efeito colateral: **Brave, OpenCode e outros processos** também passam
pelo gateway. Parar o proxy sem restaurar o WinINET derruba a rede desses processos, inclusive a
sessão do agente que está diagnosticando.

### 3.3 Feature flags e harness local

O `settings.json` do Figma 126.11.8 tinha **45 feature flags**, todas `true`, mas **sem**
`desktop_make_local_enabled`. No `main.js`:

```js
function jt(){ return je("desktop_make_local_enabled") && !mo().disableLocalMake }
...
async start(e){ if(!jt()) throw new Error("Make Local is disabled"); ... }
```

Sem a flag, `jt()` é `false` e o `figcode` não sobe. Além disso:

```text
%APPDATA%\Figma\Figcode       → não existe
%APPDATA%\Figma\FigcodeData   → não existe
```

O `figcode` nunca foi instalado/executado nesta máquina. O `figma_agent.exe` não é o `figcode`.

### 3.4 O que é o `figma_agent.exe`

Binário Rust/actix (126.11.8), escutando em `127.0.0.1:44950` e `127.0.0.1:44960`. As rotas
identificadas nas strings são:

```text
/figma/health
/figma/desktop/can-open-url
/desktop/open-url
/desktop/set-open-in-desktop-app-pref
/font-files
/font-file
/font-preview
/spelling/check
/spelling/suggest
/get-languages
/set-language
/add-words
/remove-words
/ignore-words
/version
/clear-data
```

Ele também tem updater próprio (`https://desktop.figma.com/agent/RELEASE.json`) e a string
`figma_agent::spellcheck`. **Não há** `figcode`, `claude`, `anthropic`, `openai`, `litellm` nem rota
de inferência. O processo é lançado pelo Explorer e registrado em
`SOFTWARE\Microsoft\Windows\CurrentVersion\Run` como "Figma Agent". É serviço nativo, não IA.

### 3.5 A aba Agents

O monitor rodou de **22:36:10 a 22:46:11** (243 amostras) enquanto uma mensagem era enviada na aba
**Agents**:

```text
22:36:55 | ... | Figcode=False,FigcodeData=False | ...;Figma->443;Figma->57995;...
22:46:11 | ... | Figcode=False,FigcodeData=False | ...;Figma->57995;...
```

- O Figma abriu novos processos de renderer (5 processos às 22:36:46–48).
- O agente respondeu, mas **nenhum `figcode`/`claude` apareceu** em nenhuma amostra.
- As pastas `Figcode`/`FigcodeData` continuaram ausentes.
- O Figma manteve conexões via gateway (`57995`), mas a inferência não passou por ele.

O registro de 2026-11-06 já havia identificado o protocolo cloud:

| Fluxo | Endpoint observado | Natureza |
|---|---|---|
| Agents | `POST /api/cortex/assistant/chat` | Chat do agente; `plainText`, `message.content`, `thread` |
| Make cloud | `POST /api/cortex/shared/figmake` | `aiChatMessages`, `rawUserChatDetails.rawUserMessage` |
| Ambos | `GET /api/agentcontrol/subscribe` (WebSocket) | `scope_key=assistant`, `chat_feature_type=figmake`, `metered_user_action=foundry`, `workload=assistant`/`make-generic` |

O servidor da Figma monta o system prompt/tools, chama o modelo e devolve os eventos pelo WebSocket.
O body do cliente **não é** um request de modelo; não há `/v1/messages` local.

### 3.6 Tentativa de forçar o Make Local

Para testar se a flag era a única barreira:

1. Backup do `settings.json` em `settings.json.bak-20261107-221831`.
2. Inserção de `"desktop_make_local_enabled": true` em `featureFlags`.
3. Abertura do Figma com o proxy no ambiente.

Resultado: o Figma **reescreveu o `settings.json` ao abrir** e removeu a flag.

```text
LastWriteTime : 07/11/2026 22:18:55
total flags   : 45   (voltou ao original, sem a flag mestre)
Figcode       : False
FigcodeData   : False
```

A flag é server-side. Não é configurável localmente, nem pela versão 126.11.7: o `main.js` das duas
versões usa a mesma condição `jt()`.

### 3.7 O teste "proxy parado + Make respondendo"

Esse teste não isola A2 por si só:

- O Figma já estava aberto antes da parada do proxy; conexões estabelecidas podem sobreviver.
- O Make cloud não depende do proxy local para a inferência; o servidor da Figma é quem chama o LLM.
- Para testar o renderer, seria necessário reiniciar o Figma com o proxy ativo e então parar o proxy.

A conclusão A3 não depende desse teste: ela vem da ausência do harness local e da flag server-side.

## 4. Hipóteses A1/A2/A3

| Hipótese | Descrição | Veredito |
|---|---|---|
| **A1** | O harness local (`figcode` → `claude`) roda, mas o `claude` ignora o proxy | **Descartada**: nenhum `figcode`/`claude` existe; pastas ausentes. |
| **A2** | O renderer (Chromium) ignora o WinINET | **Descartada**: Figma fala com `57995` após reiniciar; Brave também. |
| **A3** | O fluxo testado não usa o harness local (roda na nuvem) | **Confirmada**: Make e Agents são server-side; a flag do Make Local não é entregue. |

## 5. Por que o gateway não funciona no cloud

O gateway é um MITM seletivo de `api.figma.com` que reescreve `/ai-gateway/.../v1/messages` para o
LiteLLM. Isso funciona porque o `claude` local é um cliente Anthropic e envia um request de modelo.

No cloud:

```text
renderer → POST /api/cortex/... ou WebSocket /api/agentcontrol/subscribe
         → servidor da Figma (system prompt, tools, modelo, estado)
         → LLM da Figma
```

O gateway vê a requisição do renderer, mas ela é **controle de sessão**, não inferência. Para trocar
o modelo seria preciso:

- reverter o protocolo proprietário (formato, tokens, streaming, tool calls, permissões);
- emular o servidor da Figma para o cliente e o cliente para o LiteLLM;
- manter o estado da sessão e das ferramentas no meio;
- refazer isso a cada mudança do protocolo.

Isso é um produto novo, frágil, e a interceptação para trocar o provedor de um agente cloud contorna
o serviço da Figma (`metered_user_action=foundry`). O `AGENTS.md` e o PRD §12 proíbem contornar
autenticação/licenciamento e tratam "IA fora do caminho do figcode" como não-objetivo.

## 6. O que funciona

1. **Make Local** (quando existir): `figcode` → `claude` → `/v1/messages` → gateway → LiteLLM. É o
   único ponto de inferência local que o gateway controla.
2. **Agente local externo + LiteLLM + Figma MCP**: Claude Code/Cursor/Cline apontados para o LiteLLM
   (o gateway intercepta a inferência local) e conectados ao Figma MCP (`https://mcp.figma.com/mcp`)
   para ler/editar o design. Não substitui o Make/Agents, mas é um fluxo sustentável hoje.
3. **Mac + Make Local**, se o `figcode` Mac estiver disponível para a conta. O registro de 2026-11-06
   indica que o `figcode` é Mac-only no beta fechado e que `/win/UPDATE.json` responde **403**.

## 7. Impacto no produto e no onboarding

- O onboarding não pode prometer "Make funciona com o proxy" no Windows enquanto o harness local não
  existir. A integração de rede (CA + WinINET + proxy + LiteLLM) continua válida, mas o teste
  ponta a ponta com IA real fica bloqueado.
- O passo 4 do wizard deve detectar a ausência de `desktop_make_local_enabled` e exibir uma mensagem
  clara ("Make local indisponível nesta conta/versão; o gateway não intercepta o Make/Agents cloud"),
  em vez de marcar a integração como ativa e prometer um teste que não pode passar.
- O E2E de F11 não pode depender do Make/Agents cloud. O critério deve ser: integração de rede +
  self-test TLS/rewrite + mensagem de indisponibilidade; o teste com IA real fica condicionado a uma
  plataforma/conta com Make Local.
- O `--proxy-server` no restart do Figma é defesa para A1/A2, mas **não** cria o harness local e não
  resolve A3.
- O WinINET global é mais amplo do que o necessário. Se o produto continuar, avaliar escopo por
  processo/entrada ou um modo que não dependa de proxy global; hoje Brave/OpenCode também passam pelo
  gateway e uma parada sem restauração derruba a rede.

## 8. Artefatos e rastreabilidade

| Item | Caminho/valor |
|---|---|
| Backup do `settings.json` (antes do teste da flag) | `C:\Users\Marcos\AppData\Roaming\Figma\settings.json.bak-20261107-221831` |
| Estado do `settings.json` após o teste | 45 flags, sem `desktop_make_local_enabled` |
| Integração do gateway | `%LOCALAPPDATA%\com.frankfigmagateway.desktop\state\integration.json` (`active`, porta 57995) |
| Log do monitor do Make | `%TEMP%\opencode\make-diag-20261107-2218.log` |
| Log do monitor da aba Agents | `%TEMP%\opencode\agents-diag-20261107-2230.log` |
| Figma instalado | `C:\Users\Marcos\AppData\Local\Figma\app-126.11.8` (e `app-126.11.7`) |
| `figma_agent.exe` | `C:\Users\Marcos\AppData\Local\FigmaAgent\figma_agent.exe` (126.11.8) |

## 9. Referências

- [`CHANGELOG.md`](CHANGELOG.md) — registro cronológico, incluindo a limitação descoberta em
  2026-11-06 e a revalidação de 2026-11-07.
- [`plan.md`](plan.md) — plano de implementação e impacto nas fases/DoD.
- [`../../docs/figma-llm-architecture.md`](../../docs/figma-llm-architecture.md) — arquitetura do
  `figcode`/Claude Code/AI Gateway e o que não é IA.
- [`../../docs/gateway-prd.md`](../../docs/gateway-prd.md) — não-objetivos, incluindo IA fora do
  caminho do `figcode`.
- [`../../AGENTS.md`](../../AGENTS.md) — regra de não contornar autenticação/licenciamento.

## 10. Decisão pendente

O diagnóstico fecha a questão técnica; a decisão de produto continua aberta:

1. **Plugin/companion com chat próprio** usando o LiteLLM (não depende do Make).
2. **Aguardar o `figcode` Windows** e revalidar quando a Figma liberar a flag/conta.
3. **Mac + Make Local**, se o `figcode` Mac estiver disponível.
4. **Agente local + LiteLLM + Figma MCP** como fluxo principal, deixando o Make/Agents cloud fora do
   escopo.

Enquanto a decisão não for tomada, o app deve **detectar e comunicar** a indisponibilidade, não
prometer a substituição do modelo do chat cloud.

# Integração — Figma + Gateway

> Como o app Tauri encaixa o gateway no fluxo do Figma, instala a integração, mantém tudo no tray e
> como reverter. Arquitetura interna em [gateway-architecture.md](gateway-architecture.md).

## 1. Topologia

```mermaid
flowchart LR
    subgraph Windows
        E[Figma.exe<br/>Electron]
        F[figcode daemon]
        C[claude<br/>Claude Code]
        A[App Tauri<br/>dashboard + tray]
        P[Proxy MITM<br/>127.0.0.1:8787]
        L[LiteLLM<br/>127.0.0.1:4000]
        PG[(Postgres<br/>opcional)]
        CA[(CA local<br/>CurrentUser\Root)]
    end

    subgraph Internet
        API[api.figma.com<br/>rotas não-IA]
        X[Provider<br/>Copilot / OpenRouter / Azure / ...]
    end

    A -->|supervisiona| P
    A -->|supervisiona| L
    A -->|supervisiona| PG
    A -->|abre com proxy| E
    E -->|proxy do processo| P
    E -->|spawn| F -->|spawn| C
    C -->|HTTPS_PROXY| P
    P -->|túnel| API
    P -->|rewrite| L
    L --> X
    P -.-> CA
```

### O que muda e o que não muda

| Item | Muda? |
|---|---|
| Arquivos instalados do Figma | Não |
| UI, fluxos, permissões, ferramentas | Não |
| Token da Figma e renovação | Não |
| Nome do modelo no seletor | Não |
| Destino da inferência | Sim |
| Configuração de rede do usuário | Não (CA no usuário; proxy só no processo do Figma) |

## 2. Fluxo ponta a ponta

```mermaid
sequenceDiagram
    autonumber
    participant U as Usuário
    participant W as Web app Figma
    participant M as Electron main
    participant F as figcode
    participant C as claude
    participant P as Proxy
    participant L as LiteLLM
    participant X as Provider

    U->>W: prompt no Make
    W->>M: start_session / send_message
    M->>F: stdio
    F->>C: spawn com ANTHROPIC_BASE_URL/TOKEN
    C->>P: POST /ai-gateway/.../v1/messages
    P->>L: POST /v1/messages + master key
    L->>X: formato do provider
    X-->>L: stream
    L-->>P: SSE Anthropic
    P-->>C: SSE Anthropic
    C-->>F: eventos
    F-->>M: stdio
    M-->>W: UI
```

## 3. Opções de interceptação

| Opção | Como | Prós | Contras | Uso |
|---|---|---|---|---|
| **A. Proxy + CA no processo** | `--proxy-server` + `HTTPS_PROXY` injetados ao abrir o Figma + CA em `CurrentUser\Root` | Sem hosts, túnel para o resto, não altera o proxy global | CA precisa ser confiada; Figma precisa ser aberto pelo app | **Padrão do app** |
| B. Proxy + `NODE_EXTRA_CA_CERTS` | Igual, mas sem trust store | Não mexe na store | Depende de env herdado | Testes |
| C. `hosts` + reverse proxy | `api.figma.com → 127.0.0.1` | Sem proxy | Afeta todo o host; frágil | Último recurso |

### Por que funciona

- O app abre o Figma com `--proxy-server=http://127.0.0.1:<porta>`; o renderer Electron/Chromium
  passa a usar o proxy sem depender do WinINET global.
- O app também injeta `HTTPS_PROXY`, `HTTP_PROXY` e `NO_PROXY` **apenas no processo do Figma**.
- O processo main passa `NODE_USE_SYSTEM_CA=1` ao figcode; o `claude` herda.
- Claude Code suporta `NODE_EXTRA_CA_CERTS` e `HTTPS_PROXY`.
- Não há certificate pinning.

> `NO_PROXY` deve incluir `localhost,127.0.0.1,::1`. Ele é injetado no processo do Figma, não no
> ambiente do usuário.

## 4. Regras de roteamento

| Host/path | Ação |
|---|---|
| `api.figma.com/ai-gateway/llm/provider/universal/*` | MITM + rewrite + auth |
| `api.figma.com/*` | MITM + forward intacto |
| `www.figma.com`, `mcp.figma.com`, `desktop.figma.com` | Túnel CONNECT |
| `127.0.0.1`, `localhost` | Túnel direto |
| Outros | Túnel CONNECT |

Host/path são configuráveis para sobreviver a mudanças do figcode (stable/staging/gov).

## 5. Instalação automatizada

O wizard do app faz, sem admin:

1. gera CA + certificado para `api.figma.com` (e variantes);
2. instala a CA em `Cert:\CurrentUser\Root`;
3. sobe o proxy e valida TLS + rewrite;
4. abre ou reinicia o Figma com `--proxy-server` e `HTTPS_PROXY` **apenas no processo dele**
   (o botão fecha o Figma atual; salve o trabalho antes);
5. marca “conectado” quando a primeira requisição de IA passa.

O app **não grava `HKCU\Environment` nem altera o WinINET**. Ferramentas de CLI (`npm`, `git`,
`opencode`, navegadores etc.) continuam com a rede normal. O botão do dashboard é “Abrir Figma com
proxy”; abrir o Figma por fora do app não injeta o gateway.

**Rollback:** remove a CA; o proxy global nunca foi alterado. Feche o Figma para voltar ao
comportamento original.

### Limpeza de versões antigas

Instalações antigas podiam gravar `HTTPS_PROXY` em `HKCU\Environment` e ativar o WinINET. No boot,
na instalação e na remoção o app:

- restaura o env original quando o snapshot antigo não aponta para o próprio gateway;
- remove valores que apontam para o gateway (inclusive snapshots envenenados);
- restaura ou desliga o WinINET legado.

### Instalação manual (fallback)

Se política corporativa bloquear o wizard, defina as variáveis **apenas no processo do Figma** e
instale a CA exportada pelo app em “Autoridades de Certificação Raiz Confiáveis” do usuário:

```text
HTTPS_PROXY=http://127.0.0.1:8787
HTTP_PROXY=http://127.0.0.1:8787
NO_PROXY=localhost,127.0.0.1,::1
```

No Windows, abra o Figma com os argumentos:

```text
--proxy-server=http://127.0.0.1:8787 --proxy-bypass-list=localhost;127.0.0.1;::1
```

## 6. Modos de operação

| Modo | Comportamento | Uso |
|---|---|---|
| `passthrough` | Reencaminha para a Figma sem traduzir | Validar instalação |
| `translate` | Traduz para o provider configurado | Uso normal |
| `fail-open` | Se o LiteLLM falhar, túnel direto para a Figma | Continuidade |
| `fail-closed` | Erro explícito | Diagnóstico |

O padrão é `translate` com `fail-closed`; fail-open é opcional (usa o modelo da Figma).

## 7. Tray, autostart e fechamento

- O app fica no tray; fechar a janela (X) não encerra.
- Menu: Abrir dashboard, Configurar Figma, Abrir LiteLLM, Reiniciar serviços, Logs, Sair.
- Autostart no login, oculto; serviços sobem sozinhos.
- **Sair** encerra proxy/LiteLLM/Postgres e restaura resíduos globais de versões antigas. O env do
  usuário e o WinINET nunca são usados pelo desenho atual; o Figma precisa ser reaberto pelo app para
  voltar a usar o gateway.
- Single instance: segunda execução foca o dashboard.

## 8. Atualizações do figcode

- O app baixa o figcode por SHA-256 e desliga o self-update do daemon.
- `claude` sai com código 42 para aplicar update; o main respawna.
- A integração é por host/path, não por binário; updates não quebram o gateway.
- Se o path mudar: ajustar a regra na config (hot reload) e avisar.

## 9. Tratamento de falhas

| Sintoma | Causa provável | Ação |
|---|---|---|
| `unable to verify the first certificate` | CA não confiada | Reinstalar CA pelo wizard |
| `407 Proxy Authentication Required` | Proxy com auth | Remover auth |
| Make não responde | SSE incompleto / LiteLLM caído | Ver logs e health |
| `400 invalid_request_error` | Campo não suportado | Ajustar config/adapter LiteLLM |
| Tool call não executa | Tradução de `tool_use` | Conferir provider e logs |
| Copilot 401 | OAuth expirado/desconectado | Reconectar no LiteLLM UI |
| Copilot “model not supported” | Plano sem o modelo | Escolher modelo permitido |
| Postgres não sobe | Porta/dados/migração | Reiniciar serviço; ver logs |
| Outras APIs quebram | Proxy interceptando demais | Garantir túnel para não-IA |
| Make sem conexão com o gateway | Figma aberto fora do app | Usar “Abrir Figma com proxy” |

## 10. Matriz de compatibilidade

| Item | Versão validada |
|---|---|
| Figma desktop | 126.11.7 |
| figcode | 20261022.212230 |
| Claude Code | 2.1.308 |
| LiteLLM | `v1.108.1` |
| Postgres | 16.x |
| Windows | 10/11 x64 |

## 11. Plano de teste

1. TLS: `claude` conecta sem erro de certificado.
2. Passthrough: Make funciona igual ao original.
3. Tradução Anthropic: texto, tools e streaming.
4. Tradução OpenAI/OpenRouter/Azure: idem.
5. Copilot: OAuth, refresh, modelo do plano e `/v1/messages`.
6. Erros: 401, 429, 500 e timeout.
7. Cancelamento no meio do stream.
8. Update do figcode e do runtime com rollback.
9. Rollback da integração (CA) e volta ao normal; o proxy global permanece intocado.

## 12. Segurança e conformidade

- Não contorna autenticação; exige conta Figma com IA.
- Não redistribui binários da Figma/Claude Code.
- Verificar ToS de Copilot/assinaturas antes de distribuir.
- Loopback apenas; segredos no Credential Manager; logs redigidos.

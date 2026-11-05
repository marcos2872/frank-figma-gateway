# PRD — frank-figma-gateway

> Gateway local que substitui a perna de inferência do agente de IA do Figma desktop por qualquer
> provedor de LLM, sem alterar o app, o bundle ou a UI.

## 1. Contexto

O Figma desktop 126.11.7 executa o recurso de IA (Make Local) através de um processo separado,
`figcode`, que embute o Claude Code e envia requisições no formato **Anthropic Messages API** para
`https://api.figma.com/ai-gateway/llm/provider/universal`. Detalhes em
[figma-llm-architecture.md](figma-llm-architecture.md).

Hoje o usuário não tem como escolher o provedor/modelo: o token é escopado pela Figma e o modelo é
roteado pelo gateway dela. A proposta é inserir um gateway local nessa perna, preservando todo o
resto do fluxo (autenticação, UI, ferramentas, streaming).

## 2. Problema

- Não existe "BYO model" no Figma desktop.
- Quem quer usar um modelo específico (OpenAI, Anthropic direto, Google, modelo local) não consegue.
- O caminho de inferência é fechado, mas **tecnicamente interceptável**: é HTTPS/SSE padrão, com trust
  de TLS configurável e sem pinning.

## 3. Objetivos

1. Permitir que o recurso de IA do Figma use um provedor/modelo configurado pelo usuário, sem patch.
2. Preservar 100% do comportamento observável pelo app: streaming, tool calls, erros, limites.
3. Ser instalável e reversível em Windows, com configuração local e segredos fora do app.
4. Sobreviver a atualizações do `figcode` sem recompilar nada.

### Não-objetivos

- Não contornar autenticação, licenciamento ou DRM.
- Não funcionar sem uma conta Figma com IA habilitada.
- Não cobrir outras superfícies de IA que rodam no servidor da Figma.
- Não modificar `app.asar`, binários ou UI.
- Não redistribuir o app da Figma.

## 4. Personas e casos de uso

| Persona | Caso de uso |
|---|---|
| Designer/dev com acesso a IA no Figma | Usar o Make com um modelo local ou de outro provedor |
| Time com restrição de dados | Direcionar a inferência para um provedor aprovado (ex.: Azure OpenAI) |
| Pesquisador | Comparar modelos no mesmo fluxo, sem mudar de ferramenta |
| Usuário offline/laboratório | Apontar para um servidor local compatível com OpenAI |

## 5. Requisitos funcionais

Prioridade: **M** (must), **S** (should), **C** (could).

| ID | Requisito | Prioridade | Critério de aceite |
|---|---|---|---|
| FR-01 | Interceptar HTTPS de `api.figma.com` para o processo `claude` sem alterar binários | M | Make funciona em modo passthrough com o proxy ativo |
| FR-02 | Encaminhar tudo que não seja `/ai-gateway/llm/provider/universal/*` sem alteração | M | Nenhuma regressão em outras chamadas a `api.figma.com` |
| FR-03 | Aceitar `POST /v1/messages` (streaming e não-streaming) no formato Anthropic | M | Resposta e SSE válidos; agente renderiza tokens |
| FR-04 | Aceitar `POST /v1/messages/count_tokens` | M | Contagem retornada no formato Anthropic, com erro/fallback definido |
| FR-05 | Traduzir Anthropic → OpenAI Chat Completions | M | Texto, system, tools e streaming funcionam no Make |
| FR-06 | Traduzir Anthropic → Anthropic (passthrough de provedor) | M | Valida o pipeline com provedor compatível |
| FR-07 | Traduzir Anthropic → Google Gemini | S | Texto e tools básicos |
| FR-08 | Suportar provedor OpenAI-compatible local (Ollama, vLLM, LM Studio) | S | Base URL configurável |
| FR-09 | Mapear modelo lógico → provedor/modelo real por configuração | M | `claude-opus-4-8` pode apontar para qualquer modelo |
| FR-10 | Fallback entre provedores/modelos em erro | S | Requisição cai para o secundário sem quebrar o turno |
| FR-11 | Normalizar erros para o formato Anthropic | M | Retry/refresh do agente não quebram |
| FR-12 | Suportar tool use / tool result (incl. MCP) na tradução | M | Agente executa ferramentas e continua o turno |
| FR-13 | Remover/emular `cache_control`, `thinking` e betas incompatíveis | M | Nenhum 400 por campo desconhecido |
| FR-14 | Configuração local (arquivo + env), com hot reload | M | Alterar mapa de modelos sem reiniciar o app |
| FR-15 | Chaves de API fora do app (env ou keychain), com redaction em logs | M | Nenhum segredo em log/arquivo do projeto |
| FR-16 | Modo passthrough total e kill switch | M | Desligar o gateway volta o app ao comportamento original |
| FR-17 | Instalador/desinstalador Windows (CA + proxy) | S | Instala, valida e remove sem deixar lixo |
| FR-18 | Observabilidade local (log estruturado, métricas por request) | S | Latência, tokens, status e modelo por request |
| FR-19 | Health check e detecção de gateway indisponível | S | App não fica travado; erro claro |
| FR-20 | Preservar headers de lifecycle/support request quando útil | C | Rastreabilidade no provedor |

## 6. Requisitos não-funcionais

| ID | Requisito | Alvo |
|---|---|---|
| NFR-01 | Latência adicional | ≤ 50 ms p95 no primeiro byte, excluindo provedor |
| NFR-02 | Streaming | Primeiro delta repassado sem buffering; backpressure correto |
| NFR-03 | Concorrência | ≥ 4 streams simultâneos sem degradação |
| NFR-04 | Segurança | Escuta só em `127.0.0.1`; CA local com validade curta e remoção fácil |
| NFR-05 | Privacidade | Sem telemetria externa; logs com prompts desligados por padrão |
| NFR-06 | Portabilidade | Windows 10/11 x64 como alvo primário; macOS/Linux como secundário |
| NFR-07 | Robustez | Falha de provedor não derruba o processo; timeout e retry limitados |
| NFR-08 | Compatibilidade | Funcionar com o `figcode` da versão atual e degradar com aviso em mudanças de path |

## 7. Entregáveis

### M0 — Descoberta e captura

- Proxy de captura com CA local.
- Relatório com corpo, headers e sequência SSE reais de `/v1/messages` e `count_tokens`.
- **Saída**: `docs/capturas/` com amostras redigidas + confirmação do host/path.

### M1 — Passthrough

- Proxy HTTPS com MITM seletivo de `api.figma.com`.
- Encaminhamento intacto de todo o resto.
- Make funcionando com o gateway no meio.
- **Saída**: binário + instruções de instalação; teste de regressão manual.

### M2 — Ingresso Anthropic + IR

- Parser de `/v1/messages` e `/v1/messages/count_tokens`.
- Modelo intermediário canônico (IR) e encoder SSE.
- Adapter Anthropic direto (passthrough de provedor).
- **Saída**: testes de contrato com fixtures reais.

### M3 — Adapter OpenAI

- Tradução request/response e streaming.
- Tool use/tool result.
- Normalização de erros e `stop_reason`.
- **Saída**: e2e com o Make usando um modelo OpenAI.

### M4 — Roteamento e fallback

- Mapa de modelos, aliases, fallback e timeouts.
- Config com hot reload.
- **Saída**: exemplo de config + testes de falha.

### M5 — Provedores adicionais

- Gemini e OpenAI-compatible local.
- **Saída**: matriz de compatibilidade.

### M6 — Empacotamento e operação

- Instalador Windows (CA, proxy, serviço/tray), desinstalador.
- Logs, métricas, health check, kill switch.
- **Saída**: release + guia de troubleshooting.

### M7 — Documentação e validação

- Guia do usuário, arquitetura final, limitações e avisos legais.
- **Saída**: docs completos + relatório de validação.

## 8. Métricas de sucesso

| Métrica | Meta |
|---|---|
| Turnos completos sem erro de contrato | ≥ 99% |
| Streams com primeiro token em < 1 s (rede local) | ≥ 95% |
| Tool calls traduzidas com sucesso | ≥ 98% |
| Regressões em tráfego não-IA | 0 |
| Tempo de instalação/desinstalação | < 5 min |

## 9. Riscos e mitigação

| Risco | Impacto | Mitigação |
|---|---|---|
| Tradução de tool calls divergir | Alto | Fixtures reais + testes de contrato + fallback |
| `cache_control`/thinking quebrarem o provedor | Médio | Sanitização por adapter |
| Mudança de path/host no figcode | Alto | Detecção de versão + regra configurável + aviso |
| MITM afetar outras APIs da Figma | Alto | Interceptar só o path de IA; túnel para o resto |
| CA local mal instalada | Médio | Diagnóstico (`/health`, teste de TLS) e rollback |
| Uso indevido / ToS | Alto | Não contornar auth; exigir entitlement; aviso legal |
| Vazamento de prompt em log | Alto | Redaction e log desligado por padrão |

## 10. Dependências

- Figma desktop com IA habilitada (feature flag `desktop_make_local_enabled`).
- Conta com permissão para o Make Local.
- `figcode` baixado e atualizado pelo próprio app.
- Windows para o alvo primário.

## 11. Questões em aberto

1. O gateway precisa responder `count_tokens` com precisão ou pode aproximar?
2. Quais betas o Claude Code envia no fluxo Make e quais são obrigatórios?
3. O host/path muda entre canais stable/staging/gov?
4. Como o agente reage a `stop_reason` e erros traduzidos?
5. Vale expor um seletor de modelo próprio (exige patch de UI) ou apenas config?
6. Qual o comportamento esperado quando o token da Figma expira no meio do stream?

## 12. Fora de escopo

- Burlar limites de plano ou autenticação.
- Suportar IA que não passa pelo `figcode`.
- Reescrever prompts de sistema ou ferramentas do agente.
- Distribuir binários da Figma ou do Claude Code.

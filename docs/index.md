# Documentação — frank-figma-gateway

Ponto de entrada dos documentos do projeto.

| Documento | Descrição |
|---|---|
| [Arquitetura da IA no Figma](figma-llm-architecture.md) | Como o app usa LLM hoje: `figcode`, Claude Code, AI Gateway, protocolos e evidências |
| [Arquitetura do gateway](gateway-architecture.md) | Proxy MITM, LiteLLM sidecar, Postgres, roteamento, segurança e testes |
| [App desktop (Tauri)](desktop-app.md) | Janelas, tray, autostart, close-to-tray, supervisão dos sidecars e empacotamento |
| [Instalação e onboarding](onboarding.md) | Instalador mínimo, tour de configuração, download/verificação de componentes |
| [Guia de desenvolvimento](dev-guide.md) | Rodar o app, testes, logs, troubleshooting e fluxo de PR |
| [Integração Figma + gateway](figma-gateway-integration.md) | Topologia, interceptação, wizard, tray, troubleshooting e plano de teste |
| [PRD](gateway-prd.md) | Requisitos, entregáveis por milestone, métricas, riscos e questões em aberto |

## Decisões registradas

- Engine de providers: **LiteLLM Proxy**, versão fixada `v1.108.1`.
- Banco: **Postgres embutido** para o perfil Copilot; SQLite apenas para estado do app.
- App: **Tauri v2** com tray, autostart e close-to-tray.
- Instalador mínimo; runtime baixado no onboarding.
- Integração com o Figma via **CA local + `HTTPS_PROXY`**, sem patch em binários.

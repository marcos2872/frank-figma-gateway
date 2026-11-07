import type { SetupStatus, StepId, StepStatus } from "./contracts";

export const setupStatusLabel: Record<SetupStatus, string> = {
  notStarted: "Não iniciado",
  inProgress: "Em andamento",
  completed: "Concluído",
  needsAttention: "Requer atenção",
};

export const stepLabel: Record<StepId, string> = {
  welcome: "Boas-vindas",
  runtime: "Runtime LiteLLM",
  provider: "Provedor",
  models: "Modelos e fallback",
  figma: "Integração com o Figma",
  finish: "Teste final",
};

export const stepStatusLabel: Record<StepStatus, string> = {
  pending: "Pendente",
  inProgress: "Em andamento",
  done: "Concluído",
  skipped: "Pulado",
  failed: "Falhou",
};

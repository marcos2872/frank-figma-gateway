import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  ResetScope,
  ServiceStatus,
  SetupResult,
  SetupState,
  StepId,
  StepOutcome,
} from "./contracts";

export type {
  AppInfo,
  ResetScope,
  ServiceStatus,
  SetupResult,
  SetupState,
  StepId,
  StepOutcome,
} from "./contracts";

export const isTauri = "__TAURI_INTERNALS__" in window;

const fallbackServices: ServiceStatus[] = [
  {
    name: "postgres",
    state: "stopped",
    detail: "perfil Copilot",
    health: "unknown",
    restartCount: 0,
  },
  {
    name: "litellm",
    state: "stopped",
    detail: null,
    health: "unknown",
    restartCount: 0,
  },
  {
    name: "proxy",
    state: "stopped",
    detail: null,
    health: "unknown",
    restartCount: 0,
  },
];

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export async function getServiceStatus(): Promise<ServiceStatus[]> {
  return invoke<ServiceStatus[]>("service_status");
}

export async function startServices(): Promise<ServiceStatus[]> {
  return invoke<ServiceStatus[]>("start_services");
}

export async function stopServices(): Promise<ServiceStatus[]> {
  return invoke<ServiceStatus[]>("stop_services");
}

export async function previewRewrite(path: string): Promise<string | null> {
  return invoke<string | null>("preview_rewrite", { path });
}

export async function installFigmaIntegration(): Promise<SetupResult> {
  return invoke<SetupResult>("install_figma_integration");
}

export async function removeFigmaIntegration(): Promise<SetupResult> {
  return invoke<SetupResult>("remove_figma_integration");
}

export async function getSetupState(): Promise<SetupState> {
  return invoke<SetupState>("setup_get_state");
}

export async function beginStep(step: StepId): Promise<SetupState> {
  return invoke<SetupState>("setup_begin_step", { step });
}

export async function completeStep(
  step: StepId,
  outcome: StepOutcome,
): Promise<SetupState> {
  return invoke<SetupState>("setup_complete_step", { step, outcome });
}

export async function setSetupOptions(options: {
  offlineMode?: boolean;
  startWithWindows?: boolean;
}): Promise<SetupState> {
  return invoke<SetupState>("setup_set_options", {
    offlineMode: options.offlineMode ?? null,
    startWithWindows: options.startWithWindows ?? null,
  });
}

export async function resetSetup(scope: ResetScope): Promise<SetupState> {
  return invoke<SetupState>("setup_reset", { scope });
}

export function browserFallback(): ServiceStatus[] {
  return fallbackServices.map((service) => ({ ...service }));
}

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

import type {
  ActiveRuntime,
  CaStatus,
  ComponentId,
  CopilotStatus,
  EnvironmentReport,
  FigmaInfo,
  IntegrationValidation,
  IntegrationView,
  Installation,
  LogLine,
  ModelSettings,
  ProviderDescriptor,
  ProviderProfileInput,
  ProviderTestResult,
  ProxySelfTest,
  ProxyStatus,
  RuntimeManifest,
  ServiceName,
} from "./contracts";

export const DEFAULT_MANIFEST_URL =
  "https://github.com/marcos2872/frank-figma-gateway/releases/download/runtime-latest/manifest.json";
export const DEFAULT_SIGNATURE_URL = `${DEFAULT_MANIFEST_URL}.sig`;

export async function getEnvironmentReport(): Promise<EnvironmentReport> {
  return invoke<EnvironmentReport>("environment_report");
}

export async function getRuntimeStatus(): Promise<ActiveRuntime> {
  return invoke<ActiveRuntime>("runtime_status");
}

export async function fetchRuntimeManifest(
  manifestUrl: string,
  signatureUrl: string,
): Promise<RuntimeManifest> {
  return invoke<RuntimeManifest>("runtime_fetch_manifest", {
    manifestUrl,
    signatureUrl,
  });
}

export async function installRuntime(
  component: ComponentId,
  manifestUrl: string,
  signatureUrl: string,
  operationId: string,
  mirrors?: string[],
): Promise<Installation> {
  return invoke<Installation>("runtime_install", {
    component,
    manifestUrl,
    signatureUrl,
    operationId,
    mirrors: mirrors ?? null,
  });
}

export async function cancelRuntime(operationId: string): Promise<void> {
  return invoke<void>("runtime_cancel", { operationId });
}

export async function getCaStatus(): Promise<CaStatus> {
  return invoke<CaStatus>("ca_status");
}

export async function installCa(): Promise<CaStatus> {
  return invoke<CaStatus>("ca_install");
}

export async function removeCa(): Promise<CaStatus> {
  return invoke<CaStatus>("ca_remove");
}

export async function exportCa(path: string): Promise<string> {
  return invoke<string>("ca_export", { path });
}

export async function getProxyStatus(): Promise<ProxyStatus> {
  return invoke<ProxyStatus>("proxy_status");
}

export async function startProxy(): Promise<ProxyStatus> {
  return invoke<ProxyStatus>("proxy_start");
}

export async function stopProxy(): Promise<ProxyStatus> {
  return invoke<ProxyStatus>("proxy_stop");
}

export async function selfTestProxy(): Promise<ProxySelfTest> {
  return invoke<ProxySelfTest>("proxy_self_test");
}

export async function getProvidersCatalog(): Promise<ProviderDescriptor[]> {
  return invoke<ProviderDescriptor[]>("providers_catalog");
}

export async function getModels(): Promise<ModelSettings> {
  return invoke<ModelSettings>("models_get");
}

export async function applyModels(settings: ModelSettings): Promise<ModelSettings> {
  return invoke<ModelSettings>("models_apply", { settings });
}

export async function saveProvider(
  profile: ProviderProfileInput,
): Promise<SetupState> {
  return invoke<SetupState>("provider_save", { profile });
}

export async function removeProvider(profileId: string): Promise<SetupState> {
  return invoke<SetupState>("provider_remove", { profileId });
}

export async function providerSecretStatus(profileId: string): Promise<boolean> {
  return invoke<boolean>("provider_secret_status", { profileId });
}

export async function testProvider(model: string): Promise<ProviderTestResult> {
  return invoke<ProviderTestResult>("provider_test", { model });
}

export async function litellmHealth(): Promise<void> {
  return invoke<void>("litellm_health");
}

export async function getFigmaEnvironment(): Promise<FigmaInfo> {
  return invoke<FigmaInfo>("figma_environment");
}

export async function installFigma(restartFigma = false): Promise<IntegrationView> {
  return invoke<IntegrationView>("figma_install", { restartFigma });
}

export async function removeFigma(): Promise<IntegrationView> {
  return invoke<IntegrationView>("figma_remove");
}

export async function validateFigma(): Promise<IntegrationValidation> {
  return invoke<IntegrationValidation>("figma_validate");
}

export async function restartFigma(): Promise<FigmaInfo> {
  return invoke<FigmaInfo>("figma_restart");
}

export async function getIntegrationStatus(): Promise<IntegrationView> {
  return invoke<IntegrationView>("integration_status");
}

export async function getCopilotStatus(): Promise<CopilotStatus> {
  return invoke<CopilotStatus>("copilot_status");
}

export async function prepareCopilot(): Promise<CopilotStatus> {
  return invoke<CopilotStatus>("copilot_prepare");
}

export async function disableCopilot(deleteData = false): Promise<CopilotStatus> {
  return invoke<CopilotStatus>("copilot_disable", { deleteData });
}

export async function disconnectCopilot(): Promise<CopilotStatus> {
  return invoke<CopilotStatus>("copilot_disconnect");
}

export async function beginCopilotAuth(): Promise<void> {
  return invoke<void>("copilot_begin_auth");
}

export async function pollCopilotAuth(): Promise<void> {
  return invoke<void>("copilot_poll_auth");
}

export async function getServiceLogs(
  service: ServiceName,
  limit = 200,
): Promise<LogLine[]> {
  return invoke<LogLine[]>("service_logs", { service, limit });
}

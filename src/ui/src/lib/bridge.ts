import { invoke } from "@tauri-apps/api/core";

export type ServiceState =
  | "stopped"
  | "starting"
  | "running"
  | "degraded"
  | "failed";

export interface ServiceStatus {
  name: string;
  state: ServiceState;
  detail?: string | null;
}

export interface AppInfo {
  name: string;
  version: string;
  models: string[];
  proxyPrefix: string;
}

export interface SetupResult {
  ok: boolean;
  message: string;
}

export const isTauri = "__TAURI_INTERNALS__" in window;

const fallbackServices: ServiceStatus[] = [
  { name: "postgres", state: "stopped", detail: "perfil Copilot" },
  { name: "litellm", state: "stopped", detail: null },
  { name: "proxy", state: "stopped", detail: null },
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

export function browserFallback(): ServiceStatus[] {
  return fallbackServices.map((service) => ({ ...service }));
}

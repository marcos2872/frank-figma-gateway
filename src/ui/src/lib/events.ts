import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ServiceStatus, SetupState } from "./contracts";

export interface WizardOpenRequest {
  mode: "reconfigure" | "figmaOnly";
}

export function onSetupState(
  callback: (state: SetupState) => void,
): Promise<UnlistenFn> {
  return listen<SetupState>("setup://state", (event) => callback(event.payload));
}

export function onServiceStatus(
  callback: (status: ServiceStatus) => void,
): Promise<UnlistenFn> {
  return listen<ServiceStatus>("service://status", (event) =>
    callback(event.payload),
  );
}

export function onWizardOpen(
  callback: (request: WizardOpenRequest) => void,
): Promise<UnlistenFn> {
  return listen<WizardOpenRequest>("wizard://open", (event) =>
    callback(event.payload),
  );
}

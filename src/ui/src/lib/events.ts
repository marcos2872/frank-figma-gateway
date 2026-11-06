import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ServiceStatus, SetupState } from "./contracts";

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

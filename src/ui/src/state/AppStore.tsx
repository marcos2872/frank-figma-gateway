import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  beginStep as beginStepCommand,
  browserFallback,
  completeStep as completeStepCommand,
  getAppInfo,
  getServiceStatus,
  getSetupState,
  installFigmaIntegration,
  isTauri,
  removeFigmaIntegration,
  resetSetup as resetSetupCommand,
  setSetupOptions,
  startServices,
  stopServices,
} from "../lib/bridge";
import { onServiceStatus, onSetupState, onWizardOpen } from "../lib/events";
import type {
  AppInfo,
  ResetScope,
  ServiceStatus,
  SetupState,
  StepId,
  StepOutcome,
} from "../lib/contracts";

export type Screen =
  | "dashboard"
  | "wizard"
  | "providers"
  | "logs"
  | "settings";

interface AppStoreValue {
  info: AppInfo | null;
  services: ServiceStatus[];
  setup: SetupState | null;
  screen: Screen;
  busy: boolean;
  message: string | null;
  setScreen: (screen: Screen) => void;
  reload: () => Promise<void>;
  startServices: () => Promise<void>;
  stopServices: () => Promise<void>;
  installFigma: () => Promise<void>;
  removeFigma: () => Promise<void>;
  beginStep: (step: StepId) => Promise<void>;
  completeStep: (step: StepId, outcome: StepOutcome) => Promise<void>;
  setOptions: (options: {
    offlineMode?: boolean;
    startWithWindows?: boolean;
  }) => Promise<void>;
  resetSetup: (scope: ResetScope) => Promise<void>;
}

const AppStoreContext = createContext<AppStoreValue | null>(null);

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function mergeService(
  current: ServiceStatus[],
  next: ServiceStatus,
): ServiceStatus[] {
  const index = current.findIndex((service) => service.name === next.name);
  if (index === -1) {
    return [...current, next];
  }
  const copy = [...current];
  copy[index] = next;
  return copy;
}

export function AppStoreProvider({ children }: { children: ReactNode }) {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [services, setServices] = useState<ServiceStatus[]>(browserFallback());
  const [setup, setSetup] = useState<SetupState | null>(null);
  const [screen, setScreen] = useState<Screen>("dashboard");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    if (!isTauri) return;
    setBusy(true);
    setMessage(null);
    try {
      const [nextServices, nextSetup] = await Promise.all([
        getServiceStatus(),
        getSetupState(),
      ]);
      setServices(nextServices);
      setSetup(nextSetup);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    let unlistenSetup: UnlistenFn | undefined;
    let unlistenServices: UnlistenFn | undefined;
    let unlistenWizard: UnlistenFn | undefined;

    async function load() {
      try {
        const [nextInfo, nextServices, nextSetup] = await Promise.all([
          getAppInfo(),
          getServiceStatus(),
          getSetupState(),
        ]);
        if (cancelled) return;
        setInfo(nextInfo);
        setServices(nextServices);
        setSetup(nextSetup);
      } catch (error) {
        if (!cancelled) setMessage(errorMessage(error));
      }
    }

    void load();

    void onSetupState((next) => {
      if (!cancelled) setSetup(next);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenSetup = unlisten;
    });

    void onServiceStatus((next) => {
      if (!cancelled) setServices((current) => mergeService(current, next));
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenServices = unlisten;
    });

    void onWizardOpen(() => {
      if (!cancelled) setScreen("wizard");
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenWizard = unlisten;
    });

    return () => {
      cancelled = true;
      unlistenSetup?.();
      unlistenServices?.();
      unlistenWizard?.();
    };
  }, []);

  async function runServiceAction(action: () => Promise<ServiceStatus[]>) {
    setBusy(true);
    setMessage(null);
    try {
      setServices(await action());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function runSetupAction(action: () => Promise<SetupState>) {
    setBusy(true);
    setMessage(null);
    try {
      setSetup(await action());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function runSetupResult(
    action: () => Promise<{ ok: boolean; message: string }>,
  ) {
    setBusy(true);
    setMessage(null);
    try {
      const result = await action();
      setMessage(result.message);
      setSetup(await getSetupState());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const value: AppStoreValue = {
    info,
    services,
    setup,
    screen,
    busy,
    message,
    setScreen,
    reload,
    startServices: () => runServiceAction(startServices),
    stopServices: () => runServiceAction(stopServices),
    installFigma: () => runSetupResult(installFigmaIntegration),
    removeFigma: () => runSetupResult(removeFigmaIntegration),
    beginStep: (step) => runSetupAction(() => beginStepCommand(step)),
    completeStep: (step, outcome) =>
      runSetupAction(() => completeStepCommand(step, outcome)),
    setOptions: (options) => runSetupAction(() => setSetupOptions(options)),
    resetSetup: (scope) => runSetupAction(() => resetSetupCommand(scope)),
  };

  return (
    <AppStoreContext.Provider value={value}>
      {children}
    </AppStoreContext.Provider>
  );
}

export function useAppStore(): AppStoreValue {
  const context = useContext(AppStoreContext);
  if (!context) {
    throw new Error("useAppStore must be used inside AppStoreProvider");
  }
  return context;
}

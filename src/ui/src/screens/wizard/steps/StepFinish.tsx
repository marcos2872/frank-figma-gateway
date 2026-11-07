import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import {
  beginStep,
  completeStep,
  getIntegrationStatus,
  litellmHealth,
  setSetupOptions,
} from "../../../lib/bridge";
import type { IntegrationView } from "../../../lib/contracts";
import { useAppStore } from "../../../state/AppStore";
import { errorMessage, type WizardStepProps } from "../shared";

export function StepFinish(_props: WizardStepProps) {
  const { setup, services, reload, setScreen } = useAppStore();
  const [autostart, setAutostart] = useState(false);
  const [health, setHealth] = useState<"checking" | "ok" | "error">("checking");
  const [integration, setIntegration] = useState<IntegrationView | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => undefined);
    litellmHealth()
      .then(() => setHealth("ok"))
      .catch(() => setHealth("error"));
    getIntegrationStatus()
      .then(setIntegration)
      .catch(() => undefined);
  }, []);

  async function toggleAutostart() {
    const next = !autostart;
    setBusy(true);
    setError(null);
    try {
      if (next) await enable();
      else await disable();
      setAutostart(next);
      await setSetupOptions({ startWithWindows: next });
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  async function finish() {
    setBusy(true);
    setError(null);
    try {
      await beginStep("finish");
      await completeStep("finish", { kind: "done" });
      await reload();
      setScreen("dashboard");
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="wizard-step-body">
      <p>Revise o estado final. Você pode reabrir a configuração pelo tray.</p>
      <ul className="wizard-checks">
        {services.map((service) => (
          <li key={service.name} className={`wizard-check ${service.state === "running" ? "check-pass" : "check-warn"}`}>
            <span className="wizard-check-status">{service.name}</span>
            <span>{service.state}{service.port ? ` · porta ${service.port}` : ""}</span>
          </li>
        ))}
        <li className={`wizard-check ${health === "ok" ? "check-pass" : "check-warn"}`}>
          <span className="wizard-check-status">LiteLLM</span>
          <span>{health === "checking" ? "verificando" : health}</span>
        </li>
        <li className={`wizard-check ${integration?.status === "active" ? "check-pass" : "check-warn"}`}>
          <span className="wizard-check-status">Figma</span>
          <span>{integration?.status ?? "não configurado"}</span>
        </li>
      </ul>
      <label className="wizard-check-line">
        <input
          type="checkbox"
          checked={autostart}
          disabled={busy}
          onChange={() => void toggleAutostart()}
        />
        Iniciar com o Windows
      </label>
      {setup?.status === "completed" ? (
        <p className="wizard-success">Configuração concluída.</p>
      ) : null}
      {error ? <p className="wizard-error">{error}</p> : null}
      <div className="wizard-step-actions">
        <button type="button" disabled={busy} onClick={() => void finish()}>
          Concluir configuração
        </button>
      </div>
    </div>
  );
}

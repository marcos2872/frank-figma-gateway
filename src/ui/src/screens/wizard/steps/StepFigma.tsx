import { useCallback, useEffect, useState } from "react";
import {
  beginStep,
  completeStep,
  getFigmaEnvironment,
  getIntegrationStatus,
  installFigma,
  removeFigma,
  restartFigma,
  validateFigma,
} from "../../../lib/bridge";
import type {
  FigmaInfo,
  IntegrationValidation,
  IntegrationView,
} from "../../../lib/contracts";
import { useAppStore } from "../../../state/AppStore";
import { errorMessage, type WizardStepProps } from "../shared";

export function StepFigma({ onNext }: WizardStepProps) {
  const { reload } = useAppStore();
  const [figma, setFigma] = useState<FigmaInfo | null>(null);
  const [integration, setIntegration] = useState<IntegrationView | null>(null);
  const [validation, setValidation] = useState<IntegrationValidation | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [nextFigma, nextIntegration] = await Promise.all([
      getFigmaEnvironment(),
      getIntegrationStatus(),
    ]);
    setFigma(nextFigma);
    setIntegration(nextIntegration);
  }, []);

  useEffect(() => {
    refresh().catch((problem) => setError(errorMessage(problem)));
  }, [refresh]);

  async function install() {
    setBusy(true);
    setError(null);
    try {
      await beginStep("figma");
      const next = await installFigma(false);
      setIntegration(next);
      await completeStep("figma", { kind: "done" });
      await reload();
      onNext();
    } catch (problem) {
      setError(errorMessage(problem));
      await refresh().catch(() => undefined);
    } finally {
      setBusy(false);
    }
  }

  async function validate() {
    setBusy(true);
    setError(null);
    try {
      const next = await validateFigma();
      setValidation(next);
      if (next.ok && integration?.status !== "active") {
        await beginStep("figma");
        await completeStep("figma", { kind: "done" });
        await reload();
        onNext();
      }
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      setIntegration(await removeFigma());
      setValidation(null);
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  async function restart() {
    setBusy(true);
    setError(null);
    try {
      setFigma(await restartFigma());
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  async function skip() {
    setBusy(true);
    setError(null);
    try {
      await beginStep("figma");
      await completeStep("figma", {
        kind: "skipped",
        reason: "configuro a integração com o Figma depois",
      });
      await reload();
      onNext();
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="wizard-step-body">
      <p>
        A integração instala a CA em <code>CurrentUser\Root</code>, sobe o proxy
        e configura <code>HTTPS_PROXY</code> por usuário.
      </p>
      {figma ? (
        <p className="wizard-meta">
          Figma: {figma.detected ? figma.path : "não encontrado"}
          {figma.version ? ` · ${figma.version}` : ""} ·{" "}
          {figma.running ? "aberto" : "fechado"}
        </p>
      ) : null}
      {integration ? (
        <ul className="wizard-checks">
          <li className={`wizard-check ${integration.caInstalled ? "check-pass" : "check-warn"}`}>
            <span className="wizard-check-status">CA</span>
            <span>{integration.caInstalled ? "instalada" : "não instalada"}</span>
          </li>
          <li className={`wizard-check ${integration.proxyRunning ? "check-pass" : "check-warn"}`}>
            <span className="wizard-check-status">Proxy</span>
            <span>
              {integration.proxyRunning
                ? `rodando na porta ${integration.proxyPort}`
                : "parado"}
            </span>
          </li>
          <li className={`wizard-check ${integration.envInstalled ? "check-pass" : "check-warn"}`}>
            <span className="wizard-check-status">Env</span>
            <span>{integration.envInstalled ? "configurado" : "não configurado"}</span>
          </li>
        </ul>
      ) : null}
      {validation ? (
        <ul className="wizard-checks">
          {validation.checks.map((check) => (
            <li key={check.id} className={`wizard-check ${check.ok ? "check-pass" : "check-fail"}`}>
              <span className="wizard-check-status">{check.ok ? "OK" : "Falhou"}</span>
              <span>{check.message}</span>
            </li>
          ))}
        </ul>
      ) : null}
      {error ? <p className="wizard-error">{error}</p> : null}
      <div className="wizard-step-actions">
        <button type="button" className="secondary" disabled={busy} onClick={() => void validate()}>
          Validar
        </button>
        <button type="button" className="secondary" disabled={busy} onClick={() => void restart()}>
          Reiniciar Figma
        </button>
        <button type="button" className="secondary" disabled={busy} onClick={() => void remove()}>
          Remover
        </button>
        <button type="button" disabled={busy} onClick={() => void install()}>
          {busy ? "Instalando integração…" : "Instalar integração"}
        </button>
        <button type="button" className="secondary" disabled={busy} onClick={() => void skip()}>
          Pular com aviso
        </button>
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import {
  beginStep,
  completeStep,
  getEnvironmentReport,
} from "../../../lib/bridge";
import type { EnvironmentReport } from "../../../lib/contracts";
import { useAppStore } from "../../../state/AppStore";
import { errorMessage, formatBytes, type WizardStepProps } from "../shared";

const statusLabel = {
  pass: "OK",
  warn: "Aviso",
  fail: "Falhou",
  unknown: "Desconhecido",
} as const;

export function StepWelcome({ onNext }: WizardStepProps) {
  const { reload } = useAppStore();
  const [report, setReport] = useState<EnvironmentReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getEnvironmentReport()
      .then((next) => {
        if (!cancelled) setReport(next);
      })
      .catch((problem) => {
        if (!cancelled) setError(errorMessage(problem));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function next() {
    setBusy(true);
    setError(null);
    try {
      await beginStep("welcome");
      await completeStep("welcome", { kind: "done" });
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
        Vamos verificar o ambiente, baixar o runtime e conectar seu provedor ao
        Figma.
      </p>
      {report ? (
        <>
          <ul className="wizard-checks">
            {report.checks.map((check) => (
              <li key={check.id} className={`wizard-check check-${check.status}`}>
                <span className="wizard-check-status">
                  {statusLabel[check.status]}
                </span>
                <span>{check.message}</span>
              </li>
            ))}
          </ul>
          <p className="wizard-meta">
            {report.os} / {report.arch} · {formatBytes(report.freeDiskBytes)} livres
            {report.blocking ? " · há bloqueios" : ""}
          </p>
          {report.warnings.length > 0 ? (
            <p className="wizard-warning">{report.warnings.join(" · ")}</p>
          ) : null}
        </>
      ) : (
        <p className="wizard-meta">Verificando ambiente…</p>
      )}
      {error ? <p className="wizard-error">{error}</p> : null}
      <div className="wizard-step-actions">
        <button
          type="button"
          disabled={busy || !report || report.blocking}
          onClick={() => void next()}
        >
          {report?.blocking ? "Corrija os bloqueios" : "Continuar"}
        </button>
      </div>
    </div>
  );
}

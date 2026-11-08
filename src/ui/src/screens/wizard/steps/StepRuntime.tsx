import { useEffect, useState } from "react";
import {
  beginStep,
  cancelRuntime,
  completeStep,
  DEFAULT_MANIFEST_URL,
  DEFAULT_SIGNATURE_URL,
  fetchRuntimeManifest,
  installRuntime,
} from "../../../lib/bridge";
import type { DownloadProgress } from "../../../lib/contracts";
import { onDownloadProgress } from "../../../lib/events";
import { useAppStore } from "../../../state/AppStore";
import {
  errorMessage,
  formatBytes,
  type WizardStepProps,
} from "../shared";

export function StepRuntime({ onNext }: WizardStepProps) {
  const { setup, reload } = useAppStore();
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [operationId, setOperationId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [latest, setLatest] = useState<string | null>(null);
  const [latestSha, setLatestSha] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchRuntimeManifest(DEFAULT_MANIFEST_URL, DEFAULT_SIGNATURE_URL)
      .then((manifest) => {
        if (cancelled) return;
        setLatest(manifest.litellm.version);
        setLatestSha(manifest.litellm.sha256);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onDownloadProgress((next) => {
      if (!cancelled) setProgress(next);
    }).then((cleanup) => {
      if (cancelled) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  async function continueInstalled() {
    setBusy(true);
    setError(null);
    try {
      await beginStep("runtime");
      await completeStep("runtime", { kind: "done" });
      await reload();
      onNext();
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  async function install() {
    const id = crypto.randomUUID();
    setOperationId(id);
    setError(null);
    try {
      await beginStep("runtime");
      await installRuntime(
        "litellm",
        DEFAULT_MANIFEST_URL,
        DEFAULT_SIGNATURE_URL,
        id,
      );
      await completeStep("runtime", { kind: "done" });
      await reload();
      onNext();
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setOperationId(null);
    }
  }

  const installed = setup?.runtime.activeLitellm;
  // O hash muda a cada build do runtime (r4 → r5), mesmo com a mesma versão.
  const installedSha = setup?.runtime.litellmSha256;
  const updateAvailable = Boolean(
    installed && latestSha && installedSha !== latestSha,
  );
  const percent =
    progress && progress.total > 0
      ? Math.round((progress.downloaded / progress.total) * 100)
      : 0;

  return (
    <div className="wizard-step-body">
      <p>
        O runtime LiteLLM é baixado sob demanda, verificado por SHA-256 e
        instalado em uma pasta versionada.
      </p>
      {installed ? (
        <p className="wizard-success">
          LiteLLM {installed} já está instalado.
          {updateAvailable
            ? latest && latest !== installed
              ? ` Atualização disponível: ${latest}.`
              : " Há um build novo do runtime disponível."
            : ""}
        </p>
      ) : null}
      {progress ? (
        <div className="wizard-progress-block">
          <div className="wizard-progress-bar">
            <span style={{ width: `${percent}%` }} />
          </div>
          <small>
            {formatBytes(progress.downloaded)} / {formatBytes(progress.total)} ·
            tentativa {progress.attempt} · {progress.phase}
          </small>
        </div>
      ) : null}
      {error ? <p className="wizard-error">{error}</p> : null}
      <div className="wizard-step-actions">
        {operationId ? (
          <button
            type="button"
            className="secondary"
            onClick={() => void cancelRuntime(operationId)}
          >
            Cancelar
          </button>
        ) : null}
        {installed ? (
          <button
            type="button"
            disabled={!updateAvailable || Boolean(operationId)}
            onClick={() => void install()}
          >
            {updateAvailable ? "Atualizar runtime" : "Runtime atualizado"}
          </button>
        ) : (
          <button
            type="button"
            disabled={Boolean(operationId)}
            onClick={() => void install()}
          >
            Baixar e instalar LiteLLM
          </button>
        )}
        {installed ? (
          <button type="button" disabled={busy} onClick={() => void continueInstalled()}>
            Continuar
          </button>
        ) : null}
      </div>
    </div>
  );
}

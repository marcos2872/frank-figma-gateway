import { useEffect, useMemo, useState } from "react";
import {
  applyModels,
  beginStep,
  completeStep,
  litellmHealth,
  startServices,
  testProvider,
} from "../../../lib/bridge";
import type { ModelRoute, ModelSettings } from "../../../lib/contracts";
import { useAppStore } from "../../../state/AppStore";
import {
  errorMessage,
  providerEnvVar,
  providerModelPrefix,
  waitForLiteLlm,
  type WizardStepProps,
} from "../shared";

const OPUS = "claude-opus-4-8";
const HAIKU = "claude-haiku-4-5";

export function StepModels({ onNext }: WizardStepProps) {
  const { setup, reload } = useAppStore();
  const provider = setup?.provider ?? null;
  const existing = setup?.models.routes ?? [];
  const prefix = provider ? providerModelPrefix(provider.kind) : "";
  const initialOpus =
    existing.find((route) => route.publicName === OPUS)?.providerModel ?? prefix;
  const initialHaiku =
    existing.find((route) => route.publicName === HAIKU)?.providerModel ?? prefix;
  const [opus, setOpus] = useState(initialOpus);
  const [haiku, setHaiku] = useState(initialHaiku);
  const [fallback, setFallback] = useState(
    Boolean(setup?.models.fallbacks[OPUS]?.length),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setOpus(initialOpus);
    setHaiku(initialHaiku);
  }, [initialOpus, initialHaiku]);

  const envVar = useMemo(
    () => (provider ? providerEnvVar(provider.kind) : null),
    [provider],
  );

  async function apply() {
    if (!provider) {
      setError("Escolha e salve um provedor antes dos modelos.");
      return;
    }
    if (!opus.trim() || !haiku.trim()) {
      setError("Informe o modelo do provedor para as duas rotas.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      const route = (publicName: string, providerModel: string): ModelRoute => ({
        publicName,
        providerModel,
        credentialName:
          provider.kind === "gitHubCopilot" ? "github-copilot" : null,
        apiKeyEnv: provider.kind === "gitHubCopilot" ? null : envVar,
        apiBase: provider.apiBase ?? null,
        apiVersion: provider.apiVersion ?? null,
        extra: {},
      });
      const settings: ModelSettings = {
        routes: [route(OPUS, opus.trim()), route(HAIKU, haiku.trim())],
        fallbacks: fallback ? { [OPUS]: [HAIKU] } : {},
        appliedAt: null,
        configRevision: null,
      };

      await beginStep("models");
      await applyModels(settings);
      await startServices();
      await waitForLiteLlm(litellmHealth);
      await testProvider(OPUS);
      await testProvider(HAIKU);
      await completeStep("models", { kind: "done" });
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
        O agente pede <code>claude-opus-4-8</code> e <code>claude-haiku-4-5</code>. O seletor do
        Make envia codenames da Figma (ex.: <code>figaro-gpt-6-sol</code>,{" "}
        <code>anthropic-claude-5.5-opus</code>, <code>aqua</code>); esses ids caem na rota
        principal.
      </p>
      {provider ? (
        <p className="wizard-meta">
          Provedor: {provider.label}
          {envVar ? ` · credencial via ${envVar}` : " · credential github-copilot"}
        </p>
      ) : (
        <p className="wizard-warning">
          Nenhum provedor salvo. Volte ao passo anterior ou pule com aviso.
        </p>
      )}
      <div className="wizard-form">
        <label className="wizard-field">
          <span>Modelo principal (claude-opus-4-8 e ids do Make)</span>
          <input
            type="text"
            value={opus}
            placeholder={`${prefix}modelo-principal`}
            onChange={(event) => setOpus(event.target.value)}
          />
        </label>
        <label className="wizard-field">
          <span>Modelo para claude-haiku-4-5</span>
          <input
            type="text"
            value={haiku}
            placeholder={`${prefix}modelo-rapido`}
            onChange={(event) => setHaiku(event.target.value)}
          />
        </label>
        <label className="wizard-check-line">
          <input
            type="checkbox"
            checked={fallback}
            onChange={(event) => setFallback(event.target.checked)}
          />
          Usar o modelo rápido como fallback do principal
        </label>
      </div>
      {error ? <p className="wizard-error">{error}</p> : null}
      <div className="wizard-step-actions">
        <button type="button" disabled={busy || !provider} onClick={() => void apply()}>
          {busy ? "Aplicando e testando…" : "Aplicar e testar"}
        </button>
      </div>
    </div>
  );
}

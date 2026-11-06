import { useEffect, useMemo, useState } from "react";
import {
  beginCopilotAuth,
  beginStep,
  completeStep,
  getCopilotStatus,
  getProvidersCatalog,
  openExternal,
  pollCopilotAuth,
  saveProvider,
} from "../../../lib/bridge";
import type {
  CopilotStatus,
  DeviceFlow,
  ProviderDescriptor,
  ProviderKind,
  ProviderProfileInput,
} from "../../../lib/contracts";
import { useAppStore } from "../../../state/AppStore";
import { errorMessage, type WizardStepProps } from "../shared";

type Values = Record<string, string>;

export function StepProvider({ onNext }: WizardStepProps) {
  const { setup, reload } = useAppStore();
  const [catalog, setCatalog] = useState<ProviderDescriptor[]>([]);
  const [kind, setKind] = useState<ProviderKind>(
    setup?.provider?.kind ?? "openRouter",
  );
  const [values, setValues] = useState<Values>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copilot, setCopilot] = useState<CopilotStatus | null>(null);
  const [flow, setFlow] = useState<DeviceFlow | null>(null);

  useEffect(() => {
    let cancelled = false;
    getProvidersCatalog()
      .then((next) => {
        if (!cancelled) setCatalog(next);
      })
      .catch((problem) => {
        if (!cancelled) setError(errorMessage(problem));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (kind === "gitHubCopilot") {
      getCopilotStatus().then(setCopilot).catch(() => undefined);
    }
  }, [kind]);

  useEffect(() => {
    if (!flow) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const status = await pollCopilotAuth(flow.deviceCode);
        if (cancelled) return;
        setCopilot(status);
        if (status.copilot.connected) {
          setFlow(null);
          await reload();
        }
      } catch (problem) {
        if (!cancelled) {
          setError(errorMessage(problem));
          setFlow(null);
        }
      }
    };
    const id = setInterval(() => void tick(), Math.max(flow.interval, 5) * 1000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [flow, reload]);

  const descriptor = useMemo(
    () => catalog.find((item) => item.kind === kind),
    [catalog, kind],
  );

  async function save() {
    if (!descriptor) return;
    setBusy(true);
    setError(null);
    try {
      const profile: ProviderProfileInput = {
        kind,
        label: descriptor.label,
        apiKey: values.apiKey ?? null,
        apiBase: values.apiBase ?? null,
        apiVersion: values.apiVersion ?? null,
        deployment: values.deployment ?? null,
      };
      await saveProvider(profile);
      await beginStep("provider");
      await completeStep("provider", { kind: "done" });
      await reload();
      onNext();
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
      await beginStep("provider");
      await completeStep("provider", {
        kind: "skipped",
        reason: "configuro o provedor depois",
      });
      await reload();
      onNext();
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  async function connectGithub() {
    setBusy(true);
    setError(null);
    try {
      setFlow(await beginCopilotAuth());
    } catch (problem) {
      setError(errorMessage(problem));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="wizard-step-body">
      <p>Escolha um provedor. O LiteLLM faz a tradução; o app só guarda a credencial.</p>
      <div className="wizard-provider-grid">
        {catalog.map((item) => (
          <button
            key={item.kind}
            type="button"
            className={`wizard-provider-card${item.kind === kind ? " is-selected" : ""}`}
            onClick={() => {
              setKind(item.kind);
              setValues({});
            }}
          >
            <strong>{item.label}</strong>
            <small>{item.description}</small>
            {item.requiresPostgres ? <em>requer Postgres</em> : null}
          </button>
        ))}
      </div>

      {descriptor ? (
        <div className="wizard-form">
          {descriptor.fields.map((field) => (
            <label key={field.id} className="wizard-field">
              <span>
                {field.label}
                {field.required ? " *" : ""}
              </span>
              <input
                type={field.secret ? "password" : "text"}
                value={values[field.id] ?? ""}
                placeholder={field.placeholder ?? undefined}
                autoComplete={field.secret ? "off" : undefined}
                onChange={(event) =>
                  setValues((current) => ({
                    ...current,
                    [field.id]: event.target.value,
                  }))
                }
              />
            </label>
          ))}
        </div>
      ) : null}

      {kind === "gitHubCopilot" ? (
        <div className="wizard-copilot">
          <p>
            O Copilot autentica pelo OAuth do GitHub. O app grava o token no
            diretório que o LiteLLM lê para falar com o Copilot.
          </p>
          {copilot?.copilot.connected ? (
            <p className="wizard-meta">
              GitHub: {copilot.copilot.githubLogin ?? "conectado"}
            </p>
          ) : null}
          {flow ? (
            <div className="wizard-meta">
              <p>
                Abra <code>{flow.verificationUri}</code> e informe o código{" "}
                <strong>{flow.userCode}</strong>. Aguardando autorização…
              </p>
              <button
                type="button"
                className="secondary"
                onClick={() => void openExternal(flow.verificationUri)}
              >
                Abrir GitHub
              </button>
            </div>
          ) : null}
          <div className="wizard-step-actions">
            <button
              type="button"
              className="secondary"
              disabled={busy || Boolean(flow) || copilot?.copilot.connected}
              onClick={() => void connectGithub()}
            >
              {copilot?.copilot.connected ? "Conectado" : "Conectar GitHub"}
            </button>
          </div>
        </div>
      ) : null}

      {setup?.provider ? (
        <p className="wizard-success">
          Provedor salvo: {setup.provider.label} ({setup.provider.profileId})
        </p>
      ) : null}
      {error ? <p className="wizard-error">{error}</p> : null}
      <div className="wizard-step-actions">
        <button type="button" className="secondary" disabled={busy} onClick={() => void skip()}>
          Pular com aviso
        </button>
        {kind !== "gitHubCopilot" ? (
          <button type="button" disabled={busy || !descriptor} onClick={() => void save()}>
            Salvar provedor
          </button>
        ) : null}
      </div>
    </div>
  );
}

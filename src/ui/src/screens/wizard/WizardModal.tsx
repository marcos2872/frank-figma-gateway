import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent,
} from "react";
import type { StepId } from "../../lib/contracts";
import { stepLabel, stepStatusLabel } from "../../lib/labels";
import { useAppStore } from "../../state/AppStore";
import "./wizard.css";

interface WizardStep {
  id: StepId;
  phase: string;
  description: string;
}

const wizardSteps: WizardStep[] = [
  {
    id: "welcome",
    phase: "F0 / F8",
    description:
      "Checagem de plataforma, espaço em disco, internet e instalação do Figma.",
  },
  {
    id: "runtime",
    phase: "F3",
    description:
      "Download, verificação de assinatura/hash, extração e ativação do runtime LiteLLM.",
  },
  {
    id: "provider",
    phase: "F5 / F9",
    description:
      "Escolha do provedor, credenciais e teste de conexão; inclui o sub-fluxo GitHub Copilot.",
  },
  {
    id: "models",
    phase: "F5",
    description:
      "Aliases claude-opus-4-8/claude-haiku-4-5, fallback e geração do config.yaml.",
  },
  {
    id: "figma",
    phase: "F6–F8",
    description:
      "CA local, proxy MITM, variáveis em HKCU e validação de TLS/rewrite.",
  },
  {
    id: "finish",
    phase: "F10 / F11",
    description:
      "Status final, teste ponta a ponta, autostart e conclusão da configuração.",
  },
];

const focusableSelector = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export function WizardModal() {
  const { screen, setScreen, setup } = useAppStore();
  const open = screen === "wizard";
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const wasOpen = useRef(false);
  const [index, setIndex] = useState(0);

  const close = useCallback(() => setScreen("dashboard"), [setScreen]);

  useEffect(() => {
    if (open && !wasOpen.current) {
      const position = setup
        ? wizardSteps.findIndex((step) => step.id === setup.currentStep)
        : 0;
      setIndex(position < 0 ? 0 : position);
    }
    wasOpen.current = open;
  }, [open, setup]);

  useEffect(() => {
    if (!open) return;

    const previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
        return;
      }

      if (event.key !== "Tab" || !dialogRef.current) return;

      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(focusableSelector),
      );
      if (focusable.length === 0) {
        event.preventDefault();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [open, close]);

  useEffect(() => {
    if (open) {
      titleRef.current?.focus();
    }
  }, [index, open]);

  if (!open) {
    return null;
  }

  function handleBackdropMouseDown(event: MouseEvent<HTMLDivElement>) {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  return (
    <div className="wizard-backdrop" onMouseDown={handleBackdropMouseDown}>
      <div
        className="wizard-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="wizard-title"
        ref={dialogRef}
        tabIndex={-1}
      >
        <header className="wizard-header">
          <div>
            <p className="wizard-eyebrow">Configuração inicial</p>
            <h2 id="wizard-title" ref={titleRef} tabIndex={-1}>
              Configurar o gateway
            </h2>
          </div>
          <button
            type="button"
            className="wizard-close"
            onClick={close}
            aria-label="Fechar configuração"
          >
            ×
          </button>
        </header>

        <div className="wizard-progress">
          <span aria-live="polite">
            Etapa {index + 1} de {wizardSteps.length}
          </span>
          <ol className="wizard-dots" aria-label="Etapas da configuração">
            {wizardSteps.map((step, position) => (
              <li
                key={step.id}
                className={
                  position === index
                    ? "wizard-dot wizard-dot-active"
                    : "wizard-dot"
                }
                aria-current={position === index ? "step" : undefined}
              >
                <button
                  type="button"
                  className="wizard-dot-button"
                  onClick={() => setIndex(position)}
                  aria-label={`Ir para ${stepLabel[step.id]}`}
                >
                  {position + 1}
                </button>
              </li>
            ))}
          </ol>
        </div>

        {!setup ? (
          <p className="wizard-loading">Carregando estado da configuração…</p>
        ) : null}

        <div className="wizard-viewport">
          <div
            className="wizard-track"
            style={{ transform: `translateX(-${index * 100}%)` }}
          >
            {wizardSteps.map((step, position) => {
              const state = setup?.steps.find(
                (candidate) => candidate.id === step.id,
              );
              return (
                <section
                  key={step.id}
                  className="wizard-slide"
                  aria-hidden={position !== index}
                >
                  <div className="wizard-slide-head">
                    <span className="badge">{step.phase}</span>
                    {state ? (
                      <span className={`badge badge-step-${state.status}`}>
                        {stepStatusLabel[state.status]}
                      </span>
                    ) : null}
                  </div>
                  <h3>{stepLabel[step.id]}</h3>
                  <p>{step.description}</p>
                  <div className="wizard-placeholder">
                    <strong>Conteúdo em implementação</strong>
                    <p>
                      Esta etapa será ligada ao backend na fase {step.phase}. O
                      estado já é persistido e retomado pelo{" "}
                      <code>setup.json</code>.
                    </p>
                  </div>
                </section>
              );
            })}
          </div>
        </div>

        <footer className="wizard-footer">
          <button type="button" className="secondary" onClick={close}>
            Fechar
          </button>
          <div className="actions">
            <button
              type="button"
              className="secondary"
              disabled={index === 0}
              onClick={() => setIndex((current) => Math.max(0, current - 1))}
            >
              Voltar
            </button>
            <button
              type="button"
              disabled={index === wizardSteps.length - 1}
              onClick={() =>
                setIndex((current) =>
                  Math.min(wizardSteps.length - 1, current + 1),
                )
              }
            >
              Avançar
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

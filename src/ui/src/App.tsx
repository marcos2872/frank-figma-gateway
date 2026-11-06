import { useEffect, useState } from "react";
import {
  browserFallback,
  getAppInfo,
  getServiceStatus,
  installFigmaIntegration,
  isTauri,
  removeFigmaIntegration,
  startServices,
  stopServices,
  type AppInfo,
  type ServiceState,
  type ServiceStatus,
} from "./lib/bridge";

const stateLabel: Record<ServiceState, string> = {
  stopped: "Parado",
  starting: "Iniciando",
  running: "Rodando",
  degraded: "Degradado",
  failed: "Falhou",
};

function StatusCard({ service }: { service: ServiceStatus }) {
  return (
    <article className={`status-card status-${service.state}`}>
      <header>
        <span className="status-dot" aria-hidden="true" />
        <strong>{service.name}</strong>
      </header>
      <p>{stateLabel[service.state]}</p>
      {service.detail ? <small>{service.detail}</small> : null}
    </article>
  );
}

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [services, setServices] = useState<ServiceStatus[]>(browserFallback());
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) {
      return;
    }

    let cancelled = false;
    async function load() {
      try {
        const [nextInfo, nextServices] = await Promise.all([
          getAppInfo(),
          getServiceStatus(),
        ]);
        if (!cancelled) {
          setInfo(nextInfo);
          setServices(nextServices);
        }
      } catch (error) {
        if (!cancelled) {
          setMessage(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function run(action: () => Promise<ServiceStatus[]>) {
    setBusy(true);
    setMessage(null);
    try {
      setServices(await action());
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function runSetup(action: () => Promise<{ ok: boolean; message: string }>) {
    setBusy(true);
    setMessage(null);
    try {
      const result = await action();
      setMessage(result.message);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="shell">
      <header className="hero">
        <img src="/logo.svg" alt="" width={56} height={56} />
        <div>
          <h1>frank-figma-gateway</h1>
          <p>
            {isTauri ? "Tauri conectado" : "Modo navegador (comandos indisponíveis)"}
            {info ? ` · v${info.version}` : ""}
          </p>
        </div>
      </header>

      <section className="panel">
        <div className="panel-head">
          <h2>Serviços</h2>
          <div className="actions">
            <button
              type="button"
              disabled={busy || !isTauri}
              onClick={() => void run(startServices)}
            >
              Iniciar
            </button>
            <button
              type="button"
              disabled={busy || !isTauri}
              onClick={() => void run(stopServices)}
            >
              Parar
            </button>
          </div>
        </div>
        <div className="status-grid">
          {services.map((service) => (
            <StatusCard key={service.name} service={service} />
          ))}
        </div>
        <small className="hint">
          Supervisor real e sidecars ainda são stub neste scaffold.
        </small>
      </section>

      <section className="panel">
        <h2>Integração com o Figma</h2>
        <p>
          Instala a CA local, configura <code>HTTPS_PROXY</code> e sobe o proxy MITM. Reversível.
        </p>
        <div className="actions">
          <button
            type="button"
            disabled={busy || !isTauri}
            onClick={() => void runSetup(installFigmaIntegration)}
          >
            Instalar integração
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy || !isTauri}
            onClick={() => void runSetup(removeFigmaIntegration)}
          >
            Remover
          </button>
        </div>
      </section>

      {info ? (
        <section className="panel">
          <h2>Modelos</h2>
          <p>{info.models.join(" · ")}</p>
          <small className="hint">Prefixo interceptado: {info.proxyPrefix}</small>
        </section>
      ) : null}

      {message ? <p className="message">{message}</p> : null}

      <footer>
        <small>
          LiteLLM ainda não é baixado neste scaffold. Veja <code>docs/onboarding.md</code>.
        </small>
      </footer>
    </main>
  );
}

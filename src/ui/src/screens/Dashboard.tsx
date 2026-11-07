import { useEffect, useState } from "react";
import { getProxyStatus, isTauri, restartFigma, selfTestProxy } from "../lib/bridge";
import type {
  ProxySelfTest,
  ProxyStatus,
  ServiceState,
  ServiceStatus,
} from "../lib/contracts";
import { errorMessage } from "../lib/errors";
import {
  setupStatusLabel,
  stepLabel,
  stepStatusLabel,
} from "../lib/labels";
import { useAppStore } from "../state/AppStore";

const serviceStateLabel: Record<ServiceState, string> = {
  stopped: "Parado",
  starting: "Iniciando",
  running: "Rodando",
  degraded: "Degradado",
  failed: "Falhou",
  stopping: "Parando",
};

function StatusCard({ service }: { service: ServiceStatus }) {
  return (
    <article className={`status-card status-${service.state}`}>
      <header>
        <span className="status-dot" aria-hidden="true" />
        <strong>{service.name}</strong>
      </header>
      <p>{serviceStateLabel[service.state]}</p>
      {service.detail ? <small>{service.detail}</small> : null}
    </article>
  );
}

export default function Dashboard() {
  const {
    info,
    services,
    setup,
    busy,
    message,
    setScreen,
    reload,
    startServices,
    stopServices,
    installFigma,
    removeFigma,
  } = useAppStore();

  const [proxy, setProxy] = useState<ProxyStatus | null>(null);
  const [selfTest, setSelfTest] = useState<ProxySelfTest | null>(null);
  const [testing, setTesting] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [restartMessage, setRestartMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    const tick = () => {
      getProxyStatus()
        .then((next) => {
          if (!cancelled) setProxy(next);
        })
        .catch(() => undefined);
    };
    tick();
    const id = setInterval(tick, 2000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  async function testProxy() {
    setTesting(true);
    try {
      setSelfTest(await selfTestProxy());
    } catch (problem) {
      setSelfTest({ ok: false, status: 0, message: errorMessage(problem) });
    } finally {
      setTesting(false);
    }
  }

  async function restartFigmaApp() {
    setRestarting(true);
    setRestartMessage(null);
    try {
      const info = await restartFigma();
      setRestartMessage(
        info.running
          ? "Figma reiniciado com HTTPS_PROXY no ambiente."
          : "Figma aberto.",
      );
    } catch (problem) {
      setRestartMessage(errorMessage(problem));
    } finally {
      setRestarting(false);
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
              onClick={() => void startServices()}
            >
              Iniciar
            </button>
            <button
              type="button"
              disabled={busy || !isTauri}
              onClick={() => void stopServices()}
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
          Sidecars supervisionados pelo app, com health check e restart automático.
        </small>
      </section>

      <section className="panel">
        <div className="panel-head">
          <h2>Proxy MITM</h2>
          <div className="actions">
            <button
              type="button"
              className="secondary"
              disabled={testing || !isTauri}
              onClick={() => void testProxy()}
            >
              {testing ? "Testando…" : "Testar proxy"}
            </button>
          </div>
        </div>
        <p className="hint">
          {proxy?.running
            ? `Rodando em 127.0.0.1:${proxy.port}`
            : "Parado"}
          {proxy
            ? ` · conexões ${proxy.connections} · requisições de IA ${proxy.aiRequests} · falhas ${proxy.failures}`
            : ""}
        </p>
        {proxy?.firstAiRequestAt ? (
          <p className="hint">Primeira requisição de IA: {proxy.firstAiRequestAt}</p>
        ) : null}
        {selfTest ? (
          <p className="hint">
            {selfTest.ok
              ? `Self-test OK (HTTP ${selfTest.status}): ${selfTest.message}`
              : `Self-test falhou: ${selfTest.message}`}
          </p>
        ) : null}
        <small className="hint">
          Use o Figma normalmente: se “requisições de IA” continuar em 0, o
          tráfego não está passando pelo proxy — reabra o Figma depois de
          instalar a integração.
        </small>
      </section>

      {setup ? (
        <section className="panel">
          <div className="panel-head">
            <h2>Configuração</h2>
            <span className={`badge badge-${setup.status}`}>
              {setupStatusLabel[setup.status]}
            </span>
          </div>
          <p className="hint">
            Passo atual: <strong>{stepLabel[setup.currentStep]}</strong> · revisão{" "}
            {setup.revision}
          </p>
          <ol className="steps">
            {setup.steps.map((step) => (
              <li
                key={step.id}
                className={`step step-${step.status}${
                  step.stale ? " step-stale" : ""
                }`}
              >
                <span className="status-dot" aria-hidden="true" />
                <span>{stepLabel[step.id]}</span>
                <small>
                  {step.stale ? "desatualizado" : stepStatusLabel[step.status]}
                </small>
              </li>
            ))}
          </ol>
          <div className="actions">
            <button
              type="button"
              disabled={busy || !isTauri}
              onClick={() => setScreen("wizard")}
            >
              Abrir configuração
            </button>
            <button
              type="button"
              className="secondary"
              disabled={busy || !isTauri}
              onClick={() => void reload()}
            >
              Recarregar estado
            </button>
          </div>
        </section>
      ) : null}

      <section className="panel">
        <h2>Integração com o Figma</h2>
        <p>
          Instala a CA local, configura <code>HTTPS_PROXY</code> e sobe o proxy MITM. Reversível.
        </p>
        <div className="actions">
          <button
            type="button"
            disabled={busy || !isTauri}
            onClick={() => void installFigma()}
          >
            Instalar integração
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy || !isTauri}
            onClick={() => void removeFigma()}
          >
            Remover
          </button>
          <button
            type="button"
            className="secondary"
            disabled={restarting || !isTauri}
            onClick={() => void restartFigmaApp()}
          >
            {restarting ? "Reiniciando…" : "Reiniciar Figma"}
          </button>
        </div>
        {restartMessage ? <p className="hint">{restartMessage}</p> : null}
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
          O runtime do LiteLLM é baixado no onboarding e verificado por assinatura
          e SHA-256.
        </small>
      </footer>
    </main>
  );
}

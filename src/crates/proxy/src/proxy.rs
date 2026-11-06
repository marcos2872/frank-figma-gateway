//! Selective MITM HTTP proxy for `api.figma.com`.
//!
//! Only the configured hosts are terminated with a local leaf certificate. All
//! other CONNECT targets are tunneled without inspection.

use crate::ca::{CaError, CertificateAuthority};
use bytes::Bytes;
use ffg_core::rewrite::rewrite_ai_path;
use futures_util::StreamExt;
use http::{HeaderName, StatusCode};
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;

type ProxyBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

#[derive(Clone)]
pub struct ProxyConfig {
    pub port: u16,
    pub ca: Arc<CertificateAuthority>,
    pub lite_llm_url: String,
    pub master_key: String,
    pub figma_upstream: String,
    pub mitm_hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySelfTest {
    pub ok: bool,
    pub status: u16,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub connections: u64,
    pub ai_requests: u64,
    pub failures: u64,
    pub first_ai_request_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Default)]
struct Stats {
    connections: AtomicU64,
    ai_requests: AtomicU64,
    failures: AtomicU64,
    first_ai_request_at: Mutex<Option<String>>,
    last_error: Mutex<Option<String>>,
}

impl Stats {
    fn mark_ai_request(&self) {
        let previous = self.ai_requests.fetch_add(1, Ordering::SeqCst);
        if previous == 0 {
            if let Ok(mut first) = self.first_ai_request_at.lock() {
                if first.is_none() {
                    *first = Some(now_utc());
                }
            }
        }
    }

    fn set_error(&self, message: impl Into<String>) {
        if let Ok(mut error) = self.last_error.lock() {
            *error = Some(message.into());
        }
    }
}

pub struct ProxyHandle {
    port: u16,
    stats: Arc<Stats>,
    cancel: CancellationToken,
    running: Arc<AtomicBool>,
}

impl ProxyHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn status(&self) -> ProxyStatus {
        ProxyStatus {
            running: self.running.load(Ordering::SeqCst),
            port: self.port,
            connections: self.stats.connections.load(Ordering::SeqCst),
            ai_requests: self.stats.ai_requests.load(Ordering::SeqCst),
            failures: self.stats.failures.load(Ordering::SeqCst),
            first_ai_request_at: self
                .stats
                .first_ai_request_at
                .lock()
                .ok()
                .and_then(|value| value.clone()),
            last_error: self
                .stats
                .last_error
                .lock()
                .ok()
                .and_then(|value| value.clone()),
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.cancel.cancel();
    }
}

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("bind: {0}")]
    Bind(#[from] std::io::Error),

    #[error("CA: {0}")]
    Ca(#[from] CaError),

    #[error("self-test: {0}")]
    SelfTest(String),
}

pub async fn self_test(port: u16, ca: &CertificateAuthority) -> Result<ProxySelfTest, ProxyError> {
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
        .map_err(|error| ProxyError::SelfTest(error.to_string()))?;
    let root = reqwest::Certificate::from_pem(ca.cert_pem().as_bytes())
        .map_err(|error| ProxyError::SelfTest(error.to_string()))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .add_root_certificate(root)
        .build()
        .map_err(|error| ProxyError::SelfTest(error.to_string()))?;
    let response = client
        .get("https://api.figma.com/ai-gateway/llm/provider/universal/health/liveliness")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| ProxyError::SelfTest(error.to_string()))?;
    let status = response.status().as_u16();
    Ok(ProxySelfTest {
        ok: response.status().is_success(),
        status,
        message: if response.status().is_success() {
            "TLS + rewrite + LiteLLM OK".to_owned()
        } else {
            format!("resposta HTTP {status}")
        },
    })
}

pub async fn start(config: ProxyConfig) -> Result<ProxyHandle, ProxyError> {
    let listener = TcpListener::bind(("127.0.0.1", config.port)).await?;
    let port = listener.local_addr()?.port();
    let leaf = if config.mitm_hosts.is_empty() {
        None
    } else {
        Some(config.ca.leaf_for(&config.mitm_hosts)?)
    };
    let acceptor = leaf
        .map(|leaf| leaf.server_config())
        .transpose()?
        .map(|config| TlsAcceptor::from(Arc::new(config)));

    let stats = Arc::new(Stats::default());
    let cancel = CancellationToken::new();
    let running = Arc::new(AtomicBool::new(true));
    let context = Arc::new(Context {
        lite_llm_url: config.lite_llm_url.trim_end_matches('/').to_owned(),
        master_key: config.master_key,
        figma_upstream: config.figma_upstream.trim_end_matches('/').to_owned(),
        mitm_hosts: config
            .mitm_hosts
            .iter()
            .map(|host| host.to_ascii_lowercase())
            .collect(),
        acceptor,
        stats: Arc::clone(&stats),
        client: reqwest::Client::builder()
            .no_proxy()
            .user_agent("frank-figma-gateway-proxy")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()),
    });

    let loop_cancel = cancel.clone();
    let loop_stats = Arc::clone(&stats);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = loop_cancel.cancelled() => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            loop_stats.connections.fetch_add(1, Ordering::SeqCst);
                            let context = Arc::clone(&context);
                            tokio::spawn(async move {
                                if let Err(error) = handle_connection(stream, context).await {
                                    // Connection errors are normal (client aborts, TLS probes).
                                    log::debug!("proxy connection: {error}");
                                }
                            });
                        }
                        Err(error) => {
                            loop_stats.set_error(format!("accept: {error}"));
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
            }
        }
    });

    Ok(ProxyHandle {
        port,
        stats,
        cancel,
        running,
    })
}

struct Context {
    lite_llm_url: String,
    master_key: String,
    figma_upstream: String,
    mitm_hosts: Vec<String>,
    acceptor: Option<TlsAcceptor>,
    stats: Arc<Stats>,
    client: reqwest::Client,
}

async fn handle_connection(stream: TcpStream, context: Arc<Context>) -> Result<(), std::io::Error> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await? == 0 {
        return Ok(());
    }

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, _port) = target
            .rsplit_once(':')
            .ok_or_else(|| std::io::Error::other("CONNECT sem host:porta"))?;
        let mut stream = reader.into_inner();
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        stream.flush().await?;

        let is_mitm = context
            .mitm_hosts
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(host));
        if is_mitm {
            if let Some(acceptor) = &context.acceptor {
                let tls = acceptor.accept(stream).await?;
                return serve_mitm(tls, context).await;
            }
        }

        let mut upstream = TcpStream::connect(target).await?;
        let mut client = stream;
        tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
        return Ok(());
    }

    if method.eq_ignore_ascii_case("GET") && target == "/__ffg/health" {
        let mut stream = reader.into_inner();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        stream.flush().await?;
        return Ok(());
    }

    let mut stream = reader.into_inner();
    let body = format!("método não suportado: {method} {target} {version}");
    let response = format!(
        "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn serve_mitm(
    tls: tokio_rustls::server::TlsStream<TcpStream>,
    context: Arc<Context>,
) -> Result<(), std::io::Error> {
    let service = service_fn(move |request| {
        let context = Arc::clone(&context);
        async move { Ok::<_, Infallible>(handle_request(request, context).await) }
    });

    Builder::new(TokioExecutor::new())
        .serve_connection_with_upgrades(TokioIo::new(tls), service)
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))
}

async fn handle_request(request: Request<Incoming>, context: Arc<Context>) -> Response<ProxyBody> {
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let ai_path = rewrite_ai_path(path_and_query);
    let (target, ai) = match &ai_path {
        Some(rewritten) => (format!("{}{}", context.lite_llm_url, rewritten), true),
        None => (
            format!("{}{}", context.figma_upstream, path_and_query),
            false,
        ),
    };

    if ai {
        context.stats.mark_ai_request();
    }

    let method = reqwest::Method::from_bytes(request.method().as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let headers = request.headers().clone();
    let body = match request.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            context.stats.failures.fetch_add(1, Ordering::SeqCst);
            context
                .stats
                .set_error(format!("corpo da requisição: {error}"));
            return full_response(StatusCode::BAD_REQUEST, "corpo inválido");
        }
    };

    let mut builder = context.client.request(method, &target);
    for (name, value) in &headers {
        if hop_by_hop(name) || name.as_str() == "host" || name.as_str() == "content-length" {
            continue;
        }
        if ai && name.as_str() == "authorization" {
            continue;
        }
        builder = builder.header(name.as_str(), value.as_bytes());
    }
    if ai {
        builder = builder.header("authorization", format!("Bearer {}", context.master_key));
    }

    let response = match builder.body(body).send().await {
        Ok(response) => response,
        Err(error) => {
            context.stats.failures.fetch_add(1, Ordering::SeqCst);
            context.stats.set_error(error.to_string());
            return full_response(StatusCode::BAD_GATEWAY, "upstream indisponível");
        }
    };

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    for (name, value) in response.headers() {
        if hop_by_hop(name)
            || matches!(
                name.as_str(),
                "content-length" | "content-encoding" | "transfer-encoding"
            )
        {
            continue;
        }
        builder = builder.header(name.as_str(), value.as_bytes());
    }

    let stream = response
        .bytes_stream()
        .map(|chunk| chunk.map(Frame::data).map_err(std::io::Error::other));
    let body = BodyExt::boxed(StreamBody::new(stream));
    builder
        .body(body)
        .unwrap_or_else(|_| full_response(StatusCode::BAD_GATEWAY, "resposta inválida"))
}

fn full_response(status: StatusCode, message: &str) -> Response<ProxyBody> {
    let body = Full::new(Bytes::from(message.to_owned()))
        .map_err(|never| match never {})
        .boxed();
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(body)
        .unwrap_or_else(|_| {
            Response::new(
                Full::new(Bytes::from_static(b"erro interno"))
                    .map_err(|never| match never {})
                    .boxed(),
            )
        })
}

fn hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "proxy-connection"
            | "keep-alive"
            | "transfer-encoding"
            | "te"
            | "trailer"
            | "upgrade"
            | "proxy-authenticate"
            | "proxy-authorization"
    )
}

fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config(ca: Arc<CertificateAuthority>, lite_llm_url: String) -> ProxyConfig {
        ProxyConfig {
            port: 0,
            ca,
            lite_llm_url,
            master_key: "master-key".to_owned(),
            figma_upstream: "https://api.figma.com".to_owned(),
            mitm_hosts: vec!["api.figma.com".to_owned()],
        }
    }

    #[tokio::test]
    async fn tunnels_non_mitm_connections() {
        let echo = TcpListener::bind(("127.0.0.1", 0)).await.expect("echo");
        let echo_port = echo.local_addr().expect("addr").port();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = echo.accept().await.expect("accept");
                tokio::spawn(async move {
                    let mut buffer = [0u8; 64];
                    if let Ok(read) = stream.read(&mut buffer).await {
                        let _ = stream.write_all(&buffer[..read]).await;
                    }
                });
            }
        });

        let ca = Arc::new(CertificateAuthority::generate().expect("ca"));
        let handle = start(config(ca, "http://127.0.0.1:9".to_owned()))
            .await
            .expect("proxy");
        let mut stream = TcpStream::connect(("127.0.0.1", handle.port()))
            .await
            .expect("connect");
        stream
            .write_all(
                format!(
                    "CONNECT 127.0.0.1:{echo_port} HTTP/1.1\r\nHost: 127.0.0.1:{echo_port}\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .expect("connect request");
        let mut response = vec![0u8; 128];
        let read = stream.read(&mut response).await.expect("response");
        assert!(String::from_utf8_lossy(&response[..read]).starts_with("HTTP/1.1 200"));
        stream.write_all(b"ping").await.expect("write");
        let mut echoed = [0u8; 4];
        stream.read_exact(&mut echoed).await.expect("echo");
        assert_eq!(&echoed, b"ping");
        handle.stop();
    }

    #[tokio::test]
    async fn rewrites_ai_requests_to_litellm() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("authorization", "Bearer master-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "msg_1",
                "content": [{"type": "text", "text": "pong"}]
            })))
            .mount(&server)
            .await;

        let ca = Arc::new(CertificateAuthority::generate().expect("ca"));
        let handle = start(config(Arc::clone(&ca), server.uri()))
            .await
            .expect("proxy");
        let proxy =
            reqwest::Proxy::all(format!("http://127.0.0.1:{}", handle.port())).expect("proxy");
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .add_root_certificate(
                reqwest::Certificate::from_pem(ca.cert_pem().as_bytes()).expect("root"),
            )
            .build()
            .expect("client");

        let response = client
            .post("https://api.figma.com/ai-gateway/llm/provider/universal/v1/messages")
            .header("authorization", "Bearer figma-token")
            .json(&serde_json::json!({
                "model": "claude-opus-4-8",
                "max_tokens": 8,
                "messages": [{"role": "user", "content": "ping"}]
            }))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .expect("request");
        assert_eq!(response.status(), 200);
        assert!(handle.status().ai_requests >= 1);
        handle.stop();
    }
}

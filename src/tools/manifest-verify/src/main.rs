//! CI/diagnostic helper: verifies a signed runtime manifest.

use ffg_core::manifest::{
    verify_manifest, ManifestSignature, RuntimeManifest, RuntimePolicy, TrustedKey,
};
use std::env;
use std::fs;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("manifest-verify: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = Arguments::parse(env::args().skip(1))?;
    let raw = fs::read(&args.manifest).map_err(|error| format!("{}: {error}", args.manifest))?;
    let signature_raw = fs::read_to_string(&args.signature)
        .map_err(|error| format!("{}: {error}", args.signature))?;
    let signature: ManifestSignature =
        serde_json::from_str(&signature_raw).map_err(|error| format!("assinatura: {error}"))?;

    let keys_raw =
        fs::read_to_string(&args.keys).map_err(|error| format!("{}: {error}", args.keys))?;
    let keys: Vec<TrustedKey> =
        serde_json::from_str(&keys_raw).map_err(|error| format!("chaves: {error}"))?;

    let policy = match args.policy {
        Some(path) => {
            let raw = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path))?;
            serde_json::from_str(&raw).map_err(|error| format!("política: {error}"))?
        }
        None => RuntimePolicy::default(),
    };

    let now = match args.now {
        Some(value) => value,
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
    };

    let manifest = verify_manifest(&raw, &signature, &keys, &args.app_version, &policy, now)
        .map_err(|error| error.to_string())?;
    print_summary(&manifest);
    Ok(())
}

fn print_summary(manifest: &RuntimeManifest) {
    println!(
        "manifesto ok: litellm {} ({} bytes, sha256 {})",
        manifest.litellm.version, manifest.litellm.size, manifest.litellm.sha256
    );
    if let Some(pgsql) = &manifest.pgsql {
        println!(
            "             pgsql {} ({} bytes, sha256 {})",
            pgsql.version, pgsql.size, pgsql.sha256
        );
    }
    if let Some(min) = &manifest.min_app_version {
        println!("             minAppVersion {min}");
    }
}

struct Arguments {
    manifest: String,
    signature: String,
    keys: String,
    app_version: String,
    policy: Option<String>,
    now: Option<u64>,
}

impl Arguments {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut manifest = None;
        let mut signature = None;
        let mut keys = None;
        let mut app_version = None;
        let mut policy = None;
        let mut now = None;
        let mut args = args.peekable();

        while let Some(argument) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("{argument} exige um valor"))?;
            match argument.as_str() {
                "--manifest" => manifest = Some(value),
                "--signature" => signature = Some(value),
                "--keys" => keys = Some(value),
                "--app-version" => app_version = Some(value),
                "--policy" => policy = Some(value),
                "--now" => {
                    now = Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| "--now deve ser epoch em segundos".to_owned())?,
                    )
                }
                other => return Err(format!("argumento desconhecido: {other}")),
            }
        }

        Ok(Self {
            manifest: manifest.ok_or("--manifest é obrigatório")?,
            signature: signature.ok_or("--signature é obrigatório")?,
            keys: keys.ok_or("--keys é obrigatório")?,
            app_version: app_version.ok_or("--app-version é obrigatório")?,
            policy,
            now,
        })
    }
}

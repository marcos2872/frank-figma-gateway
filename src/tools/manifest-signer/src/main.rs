//! CI helper: signs `manifest.json` with an Ed25519 private key.
//!
//! The private key is provided through a file (from a CI secret) and is never
//! committed. The output is a detached signature JSON with `keyId`.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use ffg_core::manifest::ManifestSignature;
use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("manifest-signer: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = Arguments::parse(env::args().skip(1))?;
    let key_raw =
        fs::read_to_string(&args.key).map_err(|error| format!("{}: {error}", args.key))?;
    let key_bytes = decode_key(&key_raw)?;
    let signing_key = SigningKey::from_bytes(&key_bytes);

    let manifest = fs::read(&args.input).map_err(|error| format!("{}: {error}", args.input))?;
    let signature = signing_key.sign(&manifest);
    let output = ManifestSignature {
        key_id: args.key_id,
        algorithm: "ed25519".to_owned(),
        signature: STANDARD.encode(signature.to_bytes()),
    };
    let json = serde_json::to_vec_pretty(&output).map_err(|error| error.to_string())?;
    fs::write(&args.output, json).map_err(|error| format!("{}: {error}", args.output))?;
    println!("assinado {} -> {}", args.input, args.output);
    Ok(())
}

fn decode_key(raw: &str) -> Result<[u8; 32], String> {
    let value = raw.trim();
    if value.len() == 64 {
        if let Ok(bytes) = hex::decode(value) {
            return bytes
                .as_slice()
                .try_into()
                .map_err(|_| "chave hex deve ter 32 bytes".to_owned());
        }
    }

    let bytes = STANDARD
        .decode(value)
        .map_err(|error| format!("chave não é hex nem base64: {error}"))?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "chave base64 deve ter 32 bytes".to_owned())
}

struct Arguments {
    key: String,
    key_id: String,
    input: String,
    output: String,
}

impl Arguments {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut key = None;
        let mut key_id = None;
        let mut input = None;
        let mut output = None;
        let mut args = args.peekable();

        while let Some(argument) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("{argument} exige um valor"))?;
            match argument.as_str() {
                "--key" => key = Some(value),
                "--key-id" => key_id = Some(value),
                "--input" => input = Some(value),
                "--output" => output = Some(value),
                other => return Err(format!("argumento desconhecido: {other}")),
            }
        }

        Ok(Self {
            key: key.ok_or("--key é obrigatório")?,
            key_id: key_id.ok_or("--key-id é obrigatório")?,
            input: input.ok_or("--input é obrigatório")?,
            output: output.ok_or("--output é obrigatório")?,
        })
    }
}

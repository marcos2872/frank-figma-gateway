//! Pure command planning for the embedded Postgres used by GitHub Copilot.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresLayout {
    pub bin_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl PostgresLayout {
    pub fn from_runtime(runtime_dir: &Path, data_dir: &Path) -> Self {
        Self {
            bin_dir: runtime_dir.join("bin"),
            data_dir: data_dir.to_path_buf(),
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.data_dir.join("PG_VERSION").is_file()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

pub fn initdb(layout: &PostgresLayout, password_file: &Path) -> CommandSpec {
    CommandSpec {
        program: layout.bin_dir.join("initdb.exe"),
        args: vec![
            "-D".to_owned(),
            layout.data_dir.display().to_string(),
            "-U".to_owned(),
            "ffg".to_owned(),
            "--pwfile".to_owned(),
            password_file.display().to_string(),
            "--auth=scram-sha-256".to_owned(),
            "--encoding=UTF8".to_owned(),
            "--locale=C".to_owned(),
        ],
        env: BTreeMap::new(),
    }
}

pub fn create_database(layout: &PostgresLayout, port: u16, password: &str) -> CommandSpec {
    let mut env = BTreeMap::new();
    env.insert("PGPASSWORD".to_owned(), password.to_owned());
    CommandSpec {
        program: layout.bin_dir.join("psql.exe"),
        args: vec![
            "-h".to_owned(),
            "127.0.0.1".to_owned(),
            "-p".to_owned(),
            port.to_string(),
            "-U".to_owned(),
            "ffg".to_owned(),
            "-d".to_owned(),
            "postgres".to_owned(),
            "-v".to_owned(),
            "ON_ERROR_STOP=1".to_owned(),
            "-c".to_owned(),
            "CREATE DATABASE litellm".to_owned(),
        ],
        env,
    }
}

pub fn server(layout: &PostgresLayout, port: u16) -> CommandSpec {
    CommandSpec {
        program: layout.bin_dir.join("postgres.exe"),
        args: vec![
            "-D".to_owned(),
            layout.data_dir.display().to_string(),
            "-p".to_owned(),
            port.to_string(),
            "-c".to_owned(),
            "listen_addresses=127.0.0.1".to_owned(),
        ],
        env: BTreeMap::new(),
    }
}

pub fn database_url(port: u16, password: &str, database: &str) -> String {
    format!(
        "postgresql://ffg:{}@127.0.0.1:{port}/{database}?sslmode=disable",
        url_encode(password)
    )
}

pub fn url_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

#[derive(Debug, Error)]
pub enum PostgresError {
    #[error("Postgres não inicializado em {0}")]
    NotInitialized(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> PostgresLayout {
        PostgresLayout::from_runtime(Path::new("runtime/pgsql/16.9"), Path::new("data/pg"))
    }

    #[test]
    fn builds_initdb_command() {
        let spec = initdb(&layout(), Path::new("data/pw"));
        assert!(spec.program.ends_with("initdb.exe"));
        assert!(spec.args.contains(&"--auth=scram-sha-256".to_owned()));
        assert!(spec.args.contains(&"--locale=C".to_owned()));
    }

    #[test]
    fn builds_database_and_server_commands() {
        let database = create_database(&layout(), 54329, "secret");
        assert_eq!(database.env.get("PGPASSWORD"), Some(&"secret".to_owned()));
        assert!(database
            .args
            .contains(&"CREATE DATABASE litellm".to_owned()));

        let server = server(&layout(), 54329);
        assert!(server.program.ends_with("postgres.exe"));
        assert!(server
            .args
            .contains(&"listen_addresses=127.0.0.1".to_owned()));
    }

    #[test]
    fn builds_database_url_and_encodes_password() {
        assert_eq!(
            database_url(54329, "a b/c", "litellm"),
            "postgresql://ffg:a%20b%2Fc@127.0.0.1:54329/litellm?sslmode=disable"
        );
    }

    #[test]
    fn detects_initialized_cluster() {
        let dir = tempfile::tempdir().expect("tempdir");
        let layout = PostgresLayout::from_runtime(dir.path(), dir.path());
        assert!(!layout.is_initialized());
        std::fs::write(layout.data_dir.join("PG_VERSION"), "16").expect("write");
        assert!(layout.is_initialized());
    }
}

//! Port allocation for loopback sidecars.

use std::io;
use std::net::TcpListener;

/// Asks the OS for a free loopback port.
pub fn allocate() -> io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

pub fn is_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_a_usable_port() {
        let port = allocate().expect("port");
        assert!(port > 0);
        assert!(is_available(port));
    }
}

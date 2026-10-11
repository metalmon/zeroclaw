//! Volt lease gate: obtain a short-lived signed lease from the local license
//! agent over a unix socket and verify it with the build-time public key.
//! Fail-closed: without a valid lease the process does not start.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Deserialize;

/// Lease envelope: `<payload_b64url>.<sig_b64url>` (see the volt-license agent).
pub const LEASE_SCHEMA: u32 = 1;
/// Agent lease TTL (mirrors the agent's LEASE_TTL_SECS).
pub const LEASE_TTL_SECS: u64 = 60;
/// How long a missing/bad lease is tolerated at runtime before exiting.
pub const LEASE_GRACE_SECS: u64 = 2 * LEASE_TTL_SECS;
/// Refresh interval while the lease is healthy.
pub const LEASE_REFRESH_SECS: u64 = 25;

/// Compile-time lease public key (base64url, 32-byte Ed25519). Injected by the
/// fork deploy build; intentionally NOT overridable at runtime — a host-side
/// env override would let an attacker point voltd at their own agent+key.
pub const LEASE_PUBLIC_KEY_B64: Option<&str> = option_env!("VOLTD_LEASE_PUBLIC_KEY");

pub const SOCKET_ENV: &str = "VOLTD_LEASE_SOCKET";
pub const LICENSE_FILE_ENV: &str = "VOLTD_LICENSE_FILE";

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

#[derive(Debug, Deserialize)]
pub struct LeasePayload {
    pub schema: u32,
    pub exp: u64,
    pub license_id: String,
    pub agent_boot: String,
}

#[derive(Debug)]
pub enum LeaseError {
    Malformed,
    BadSignature,
    Expired { exp: u64, now: u64 },
    Unreachable { socket: String, err: String },
    AgentError { err: String },
}

/// Verify a lease envelope against the Ed25519 public key (32 raw bytes).
pub fn verify_lease_envelope(
    envelope: &str,
    key: &[u8; 32],
    now: u64,
) -> std::result::Result<LeasePayload, LeaseError> {
    let (p_b64, sig_b64) = envelope
        .trim()
        .split_once('.')
        .ok_or(LeaseError::Malformed)?;
    let payload_bytes = B64
        .decode(p_b64.trim())
        .map_err(|_| LeaseError::Malformed)?;
    let sig = B64
        .decode(sig_b64.trim())
        .map_err(|_| LeaseError::Malformed)?;
    let key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key);
    key.verify(&payload_bytes, &sig)
        .map_err(|_| LeaseError::BadSignature)?;
    let payload: LeasePayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| LeaseError::Malformed)?;
    if payload.schema != LEASE_SCHEMA {
        return Err(LeaseError::Malformed);
    }
    if now >= payload.exp {
        return Err(LeaseError::Expired {
            exp: payload.exp,
            now,
        });
    }
    Ok(payload)
}

pub fn parse_public_key(b64: &str) -> std::result::Result<[u8; 32], LeaseError> {
    let bytes = B64.decode(b64.trim()).map_err(|_| LeaseError::Malformed)?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| LeaseError::Malformed)
}

/// One request/response over the agent socket: `{"op":"lease"}` in,
/// `{"lease":"<envelope>"}` or `{"error":"<text>"}` out.
pub async fn request_lease(socket: &Path) -> std::result::Result<String, LeaseError> {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        request_lease_once(socket),
    )
    .await
    .map_err(|_| LeaseError::Unreachable {
        socket: socket.display().to_string(),
        err: "timeout".into(),
    })?
}

async fn request_lease_once(socket: &Path) -> std::result::Result<String, LeaseError> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    let socket_display = socket.display().to_string();
    let unreachable_err = |err: String| LeaseError::Unreachable {
        socket: socket_display.clone(),
        err,
    };
    let mut stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|err| unreachable_err(err.to_string()))?;
    let (read_half, mut write_half) = stream.split();
    let request = serde_json::json!({ "op": "lease" }).to_string();
    write_half
        .write_all(request.as_bytes())
        .await
        .map_err(|err| unreachable_err(err.to_string()))?;
    write_half
        .write_all(b"\n")
        .await
        .map_err(|err| unreachable_err(err.to_string()))?;
    write_half
        .flush()
        .await
        .map_err(|err| unreachable_err(err.to_string()))?;
    let mut line = String::new();
    let mut reader = BufReader::new(read_half.take(8_192));
    match reader.read_line(&mut line).await {
        Ok(0) => return Err(unreachable_err("agent closed the connection".into())),
        Ok(_) => {}
        Err(err) => return Err(unreachable_err(err.to_string())),
    }
    let resp: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|_| LeaseError::AgentError {
            err: "bad response".into(),
        })?;
    match resp.get("lease").and_then(|v| v.as_str()) {
        Some(lease) => Ok(lease.to_string()),
        None => Err(LeaseError::AgentError {
            err: resp
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
        }),
    }
}

/// Resolve the agent socket path from the environment (set by the deployment).
pub fn socket_path() -> Option<PathBuf> {
    match std::env::var(SOCKET_ENV) {
        Ok(v) if !v.trim().is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keypair() -> (ed25519_dalek::SigningKey, ed25519_dalek::VerifyingKey) {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        (sk, sk.verifying_key())
    }

    fn signed_lease(sk: &ed25519_dalek::SigningKey, exp: u64) -> String {
        use ed25519_dalek::Signer;
        let payload = serde_json::json!({
            "schema": LEASE_SCHEMA,
            "exp": exp,
            "license_id": "lic",
            "agent_boot": "boot-1",
        });
        let bytes = serde_json::to_vec(&payload).unwrap();
        format!(
            "{}.{}",
            B64.encode(&bytes),
            B64.encode(&sk.sign(&bytes).to_bytes())
        )
    }

    #[test]
    fn verify_accepts_valid_lease() {
        let (sk, vk) = keypair();
        let key: [u8; 32] = vk.to_bytes();
        let lease = verify_lease_envelope(&signed_lease(&sk, 2_000), &key, 1_000).unwrap();
        assert_eq!(lease.license_id, "lic");
        assert_eq!(lease.exp, 2_000);
    }

    #[test]
    fn verify_rejects_wrong_key_and_garbage() {
        let (sk, _) = keypair();
        let other_vk = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]).verifying_key();
        let key: [u8; 32] = other_vk.to_bytes();
        assert!(matches!(
            verify_lease_envelope(&signed_lease(&sk, 2_000), &key, 1_000),
            Err(LeaseError::BadSignature)
        ));
        assert!(matches!(
            verify_lease_envelope("garbage", &key, 1_000),
            Err(LeaseError::Malformed)
        ));
        assert!(matches!(
            verify_lease_envelope("a.b", &key, 1_000),
            Err(LeaseError::Malformed)
        ));
    }

    #[test]
    fn verify_rejects_expired_lease() {
        let (sk, vk) = keypair();
        let key: [u8; 32] = vk.to_bytes();
        assert!(matches!(
            verify_lease_envelope(&signed_lease(&sk, 1_000), &key, 1_000),
            Err(LeaseError::Expired { .. })
        ));
    }

    #[test]
    fn parse_public_key_roundtrip() {
        let (_, vk) = keypair();
        let text = B64.encode(vk.to_bytes());
        let key = parse_public_key(&text).unwrap();
        assert_eq!(key, vk.to_bytes());
        assert!(parse_public_key("!!").is_err());
    }

    fn serve_lease_socket(
        response: String,
    ) -> (tempfile::TempDir, PathBuf, std::thread::JoinHandle<()>) {
        use std::io::{BufRead, BufReader, Write};
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("lease.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let serve = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut line = String::new();
                if BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .is_ok()
                {
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        });
        (dir, socket, serve)
    }

    #[tokio::test]
    async fn request_lease_over_socket() {
        let (sk, vk) = keypair();
        let lease = signed_lease(&sk, 4_000_000_000);
        let (_dir, socket, serve) = serve_lease_socket(format!("{{\"lease\":\"{lease}\"}}\n"));
        let got = request_lease(&socket).await.unwrap();
        let key: [u8; 32] = vk.to_bytes();
        assert!(verify_lease_envelope(&got, &key, 1_000).is_ok());
        let _ = serve.join();
    }

    #[tokio::test]
    async fn request_lease_reports_agent_error() {
        let (_dir, socket, serve) = serve_lease_socket("{\"error\":\"nope\"}\n".into());
        let err = request_lease(&socket).await.unwrap_err();
        assert!(matches!(err, LeaseError::AgentError { .. }));
        let _ = serve.join();
    }
}

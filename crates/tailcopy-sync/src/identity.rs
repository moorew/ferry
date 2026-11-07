//! Identity keys, the session proof and pairing links (SPEC 21.3).
//!
//! Desktops and relays hold an Ed25519 key. A phone can't ask Tailscale who is at
//! an address, so it pins each desktop's key and checks, on every session, that the
//! listener signed its challenge.

use std::fmt;
use std::net::SocketAddr;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use tailcopy_core::DeviceId;
use tailcopy_core::names::PAIR_URL_SCHEME;

const PROOF_CONTEXT: &[u8] = b"tailcopy-session-v1";

/// This node's identity key.
pub struct Identity {
    signing: SigningKey,
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Identity({})", encode_key(&self.public_key()))
    }
}

impl Identity {
    pub fn generate() -> anyhow::Result<Identity> {
        Ok(Identity::from_seed(random_32()?))
    }

    pub fn from_seed(seed: [u8; 32]) -> Identity {
        Identity {
            signing: SigningKey::from_bytes(&seed),
        }
    }

    /// Loads the 32-byte seed at `path`, or creates one readable only by this user.
    pub fn load_or_create(path: &Path) -> anyhow::Result<Identity> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let seed: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                    anyhow::anyhow!("{} isn't a 32-byte identity key", path.display())
                })?;
                Ok(Identity::from_seed(seed))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let id = Identity::generate()?;
                write_private(path, &id.signing.to_bytes())?;
                tracing::info!("created an identity key");
                Ok(id)
            }
            Err(e) => Err(anyhow::anyhow!("couldn't read {}: {e}", path.display())),
        }
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Signs a dialler's challenge as the listener `me` (SPEC 21.3.4).
    pub fn prove(&self, challenge: &[u8; 32], me: &DeviceId, dialer: &DeviceId) -> [u8; 64] {
        self.signing
            .sign(&proof_message(challenge, me, dialer))
            .to_bytes()
    }
}

fn proof_message(challenge: &[u8; 32], listener: &DeviceId, dialer: &DeviceId) -> Vec<u8> {
    let mut m = Vec::with_capacity(PROOF_CONTEXT.len() + 34 + listener.0.len() + dialer.0.len());
    m.extend_from_slice(PROOF_CONTEXT);
    m.push(0);
    m.extend_from_slice(challenge);
    m.extend_from_slice(listener.0.as_bytes());
    m.push(0);
    m.extend_from_slice(dialer.0.as_bytes());
    m
}

/// Whether `proof` is `listener`'s signature, under `key`, of the challenge this
/// dialler sent.
pub fn verify_proof(
    key: &[u8; 32],
    challenge: &[u8; 32],
    listener: &DeviceId,
    dialer: &DeviceId,
    proof: &[u8],
) -> bool {
    let (Ok(key), Ok(sig)) = (VerifyingKey::from_bytes(key), Signature::from_slice(proof)) else {
        return false;
    };
    key.verify_strict(&proof_message(challenge, listener, dialer), &sig)
        .is_ok()
}

pub fn new_challenge() -> anyhow::Result<[u8; 32]> {
    random_32()
}

fn random_32() -> anyhow::Result<[u8; 32]> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| anyhow::anyhow!("no system randomness: {e}"))?;
    Ok(b)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    // The data directory is under the user's profile, which only they can read.
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}

pub fn encode_key(key: &[u8; 32]) -> String {
    URL_SAFE_NO_PAD.encode(key)
}

pub fn decode_key(s: &str) -> Option<[u8; 32]> {
    URL_SAFE_NO_PAD.decode(s).ok()?.try_into().ok()
}

/// What a pairing QR code carries (SPEC 21.3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairLink {
    pub device_id: DeviceId,
    pub display_name: String,
    pub key: [u8; 32],
    pub addrs: Vec<SocketAddr>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PairLinkError {
    #[error("this isn't a TailCopy pairing code")]
    NotPairing,
    #[error("this pairing code is from a newer version of TailCopy")]
    UnknownVersion,
    #[error("this pairing code is damaged")]
    Malformed,
}

impl PairLink {
    pub fn to_url(&self) -> String {
        let mut q = url::form_urlencoded::Serializer::new(String::new());
        q.append_pair("v", "1")
            .append_pair("id", &self.device_id.0)
            .append_pair("name", &self.display_name)
            .append_pair("key", &encode_key(&self.key));
        for a in &self.addrs {
            q.append_pair("addr", &a.to_string());
        }
        format!("{PAIR_URL_SCHEME}://pair?{}", q.finish())
    }

    pub fn parse(link: &str) -> Result<PairLink, PairLinkError> {
        let url = url::Url::parse(link.trim()).map_err(|_| PairLinkError::NotPairing)?;
        if url.scheme() != PAIR_URL_SCHEME || url.host_str() != Some("pair") {
            return Err(PairLinkError::NotPairing);
        }
        let (mut v, mut id, mut name, mut key, mut addrs) = (None, None, None, None, Vec::new());
        for (k, val) in url.query_pairs() {
            match &*k {
                "v" => v = Some(val.into_owned()),
                "id" => id = Some(val.into_owned()),
                "name" => name = Some(val.into_owned()),
                "key" => key = decode_key(&val),
                "addr" => addrs.push(val.parse().map_err(|_| PairLinkError::Malformed)?),
                _ => {}
            }
        }
        match v.as_deref() {
            Some("1") => {}
            Some(_) => return Err(PairLinkError::UnknownVersion),
            None => return Err(PairLinkError::Malformed),
        }
        let id = id
            .filter(|s| !s.is_empty())
            .ok_or(PairLinkError::Malformed)?;
        let key = key.ok_or(PairLinkError::Malformed)?;
        if addrs.is_empty() {
            return Err(PairLinkError::Malformed);
        }
        Ok(PairLink {
            device_id: DeviceId(id),
            display_name: name.unwrap_or_default(),
            key,
            addrs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proof_verifies_only_for_its_challenge_and_ids() {
        let id = Identity::from_seed([1; 32]);
        let (desk, phone) = (DeviceId::from("desk"), DeviceId::from("phone"));
        let ch = [9; 32];
        let proof = id.prove(&ch, &desk, &phone);
        let key = id.public_key();
        assert!(verify_proof(&key, &ch, &desk, &phone, &proof));
        assert!(!verify_proof(&key, &[8; 32], &desk, &phone, &proof));
        assert!(!verify_proof(&key, &ch, &phone, &desk, &proof));
        assert!(!verify_proof(
            &key,
            &ch,
            &desk,
            &DeviceId::from("other"),
            &proof
        ));
        let other = Identity::from_seed([2; 32]).public_key();
        assert!(!verify_proof(&other, &ch, &desk, &phone, &proof));
        assert!(!verify_proof(&key, &ch, &desk, &phone, &proof[..63]));
    }

    #[test]
    fn pair_links_round_trip() {
        let link = PairLink {
            device_id: DeviceId::from("019a0000-0000-7000-8000-000000000001"),
            display_name: "Will's desk & co".into(),
            key: Identity::from_seed([3; 32]).public_key(),
            addrs: vec![
                "100.64.0.1:7441".parse().unwrap(),
                "[fd7a:115c:a1e0::1]:7441".parse().unwrap(),
            ],
        };
        let url = link.to_url();
        assert!(url.starts_with("tailcopy://pair?v=1&"));
        assert_eq!(PairLink::parse(&url).unwrap(), link);
    }

    #[test]
    fn bad_pair_links_are_refused() {
        let key = encode_key(&[4; 32]);
        let ok = format!("tailcopy://pair?v=1&id=d&key={key}&addr=100.64.0.1:7441");
        assert!(PairLink::parse(&ok).is_ok());
        for (link, err) in [
            (
                "https://example.com/pair?v=1".to_string(),
                PairLinkError::NotPairing,
            ),
            ("not a url".into(), PairLinkError::NotPairing),
            (ok.replace("v=1", "v=2"), PairLinkError::UnknownVersion),
            (ok.replace(&key, "abc"), PairLinkError::Malformed),
            (
                ok.replace("&addr=100.64.0.1:7441", ""),
                PairLinkError::Malformed,
            ),
            (
                ok.replace("100.64.0.1:7441", "nonsense"),
                PairLinkError::Malformed,
            ),
            (ok.replace("id=d", "id="), PairLinkError::Malformed),
        ] {
            assert_eq!(PairLink::parse(&link), Err(err), "{link}");
        }
    }

    #[test]
    fn keys_persist_and_bad_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity.key");
        let a = Identity::load_or_create(&path).unwrap();
        let b = Identity::load_or_create(&path).unwrap();
        assert_eq!(a.public_key(), b.public_key());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::write(&path, b"short").unwrap();
        assert!(Identity::load_or_create(&path).is_err());
    }
}

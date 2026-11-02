//! Data model: identifiers, ops and devices (SPEC 6.1, 6.3).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_bytes::ByteArray;

use crate::error::{Error, Result};

/// A 32-byte BLAKE3 digest, serialized as MessagePack bin via `serde_bytes`.
pub type Hash32 = ByteArray<32>;

/// BLAKE3 of `bytes`.
pub fn blake3_hash(bytes: &[u8]) -> Hash32 {
    ByteArray::new(*blake3::hash(bytes).as_bytes())
}

/// Lower-case hex of a digest.
pub fn hash_hex(hash: &Hash32) -> String {
    blake3::Hash::from_bytes(hash.into_array())
        .to_hex()
        .to_string()
}

/// Reads a 32-byte digest from a byte slice.
pub fn hash_from_slice(bytes: &[u8]) -> Result<Hash32> {
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::Invalid("hash is not 32 bytes".into()))?;
    Ok(ByteArray::new(arr))
}

fn uuid_v7_at(now_ms: u64) -> String {
    let ts = uuid::Timestamp::from_unix(
        uuid::NoContext,
        now_ms / 1000,
        ((now_ms % 1000) * 1_000_000) as u32,
    );
    uuid::Uuid::new_v7(ts).to_string()
}

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// A new UUIDv7 using `now_ms` for its timestamp.
            pub fn new_v7(now_ms: u64) -> Self {
                Self(uuid_v7_at(now_ms))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

string_id!(
    /// Identifies a device (an op origin). UUIDv7 string.
    DeviceId
);
string_id!(
    /// Identifies a clip. UUIDv7 string, new for every `ClipAdded`.
    ClipId
);

/// Highest contiguous `seq` held per origin (SPEC 6.1).
pub type VersionVector = BTreeMap<DeviceId, u64>;

/// What a clip holds, for display and paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClipKind {
    Text,
    Rich,
    Image,
}

impl ClipKind {
    /// Value used in the `clips.kind` column.
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Rich => "rich",
            ClipKind::Image => "image",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(ClipKind::Text),
            "rich" => Some(ClipKind::Rich),
            "image" => Some(ClipKind::Image),
            _ => None,
        }
    }
}

/// MIME types TailCopy stores (SPEC 9.6).
pub mod mime {
    pub const TEXT: &str = "text/plain";
    pub const HTML: &str = "text/html";
    pub const RTF: &str = "text/rtf";
    pub const PNG: &str = "image/png";
}

/// One stored format of a clip, pointing at a content-addressed blob.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FormatRef {
    pub mime: String,
    pub blob_hash: Hash32,
    pub size: u64,
}

/// One entry in an origin's append-only log (SPEC 6.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op {
    pub origin: DeviceId,
    /// Per-origin, starts at 1, gapless at the origin.
    pub seq: u64,
    pub hlc: u64,
    pub body: OpBody,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpBody {
    ClipAdded {
        clip_id: ClipId,
        kind: ClipKind,
        group_key: Hash32,
        /// Max 500 chars; empty in a stub.
        preview: String,
        /// Empty in a stub.
        formats: Vec<FormatRef>,
        /// Only when `sync_source_app` is on.
        #[serde(default)]
        source_app: Option<String>,
        /// Stub left by retention or deletion (SPEC 6.5).
        #[serde(default)]
        trimmed: bool,
    },
    /// Pasted or copied from the picker or manager; moves the clip to the top.
    ClipUsed {
        clip_id: ClipId,
    },
    PinSet {
        clip_id: ClipId,
        pinned: bool,
    },
    /// Max 120 chars.
    LabelSet {
        clip_id: ClipId,
        label: Option<String>,
    },
    ClipDeleted {
        clip_id: ClipId,
    },
    HistoryCleared {
        before_hlc: u64,
        include_pinned: bool,
    },
}

/// Integer stored in `ops.kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum OpKind {
    ClipAdded = 1,
    ClipUsed = 2,
    PinSet = 3,
    LabelSet = 4,
    ClipDeleted = 5,
    HistoryCleared = 6,
}

impl OpBody {
    pub fn kind(&self) -> OpKind {
        match self {
            OpBody::ClipAdded { .. } => OpKind::ClipAdded,
            OpBody::ClipUsed { .. } => OpKind::ClipUsed,
            OpBody::PinSet { .. } => OpKind::PinSet,
            OpBody::LabelSet { .. } => OpKind::LabelSet,
            OpBody::ClipDeleted { .. } => OpKind::ClipDeleted,
            OpBody::HistoryCleared { .. } => OpKind::HistoryCleared,
        }
    }

    /// The clip this op is about; `None` for `HistoryCleared`.
    pub fn clip_id(&self) -> Option<&ClipId> {
        match self {
            OpBody::ClipAdded { clip_id, .. }
            | OpBody::ClipUsed { clip_id }
            | OpBody::PinSet { clip_id, .. }
            | OpBody::LabelSet { clip_id, .. }
            | OpBody::ClipDeleted { clip_id } => Some(clip_id),
            OpBody::HistoryCleared { .. } => None,
        }
    }
}

impl Op {
    /// MessagePack with field names (`rmp_serde::to_vec_named`), as on the wire.
    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Op> {
        Ok(rmp_serde::from_slice(bytes)?)
    }

    /// This op rewritten as a stub: preview and formats cleared, `trimmed = true`.
    /// Ops other than `ClipAdded` are returned unchanged.
    pub fn to_stub(&self) -> Op {
        let mut op = self.clone();
        if let OpBody::ClipAdded {
            preview,
            formats,
            source_app,
            trimmed,
            ..
        } = &mut op.body
        {
            preview.clear();
            formats.clear();
            *source_app = None;
            *trimmed = true;
        }
        op
    }

    pub fn is_stub(&self) -> bool {
        matches!(self.body, OpBody::ClipAdded { trimmed: true, .. })
    }
}

/// Maximum label length in chars (SPEC 6.3).
pub const MAX_LABEL_CHARS: usize = 120;
/// Maximum preview length in chars (SPEC 6.2, 6.3).
pub const MAX_PREVIEW_CHARS: usize = 500;

/// What kind of node a device is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Device,
    Relay,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Device => "device",
            Role::Relay => "relay",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "device" => Some(Role::Device),
            "relay" => Some(Role::Relay),
            _ => None,
        }
    }
}

/// A row of the `devices` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: DeviceId,
    pub display_name: String,
    pub role: Role,
    pub ts_node_id: Option<String>,
    pub ts_hostname: Option<String>,
    pub last_ts_ip: Option<String>,
    pub last_seen_ms: Option<u64>,
    pub is_self: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ops() -> Vec<Op> {
        let clip = ClipId::new_v7(1_700_000_000_000);
        let origin = DeviceId::new_v7(1_700_000_000_000);
        let bodies = vec![
            OpBody::ClipAdded {
                clip_id: clip.clone(),
                kind: ClipKind::Rich,
                group_key: blake3_hash(b"x"),
                preview: "hello".into(),
                formats: vec![FormatRef {
                    mime: mime::TEXT.into(),
                    blob_hash: blake3_hash(b"hello"),
                    size: 5,
                }],
                source_app: Some("editor".into()),
                trimmed: false,
            },
            OpBody::ClipUsed {
                clip_id: clip.clone(),
            },
            OpBody::PinSet {
                clip_id: clip.clone(),
                pinned: true,
            },
            OpBody::LabelSet {
                clip_id: clip.clone(),
                label: Some("l".into()),
            },
            OpBody::ClipDeleted {
                clip_id: clip.clone(),
            },
            OpBody::HistoryCleared {
                before_hlc: 42,
                include_pinned: true,
            },
        ];
        bodies
            .into_iter()
            .enumerate()
            .map(|(i, body)| Op {
                origin: origin.clone(),
                seq: i as u64 + 1,
                hlc: 1000 + i as u64,
                body,
            })
            .collect()
    }

    #[test]
    fn op_round_trips_through_msgpack() {
        for op in sample_ops() {
            let bytes = op.encode().unwrap();
            assert_eq!(Op::decode(&bytes).unwrap(), op);
        }
    }

    #[test]
    fn hashes_encode_as_msgpack_bin() {
        let op = &sample_ops()[0];
        let bytes = op.encode().unwrap();
        // bin8 header (0xc4) followed by length 32 must appear: serde_bytes, not an array.
        assert!(bytes.windows(2).any(|w| w == [0xc4, 32]));
    }

    #[test]
    fn missing_serde_default_fields_decode() {
        #[derive(Serialize)]
        enum OldBody {
            ClipAdded {
                clip_id: ClipId,
                kind: ClipKind,
                group_key: Hash32,
                preview: String,
                formats: Vec<FormatRef>,
            },
        }
        #[derive(Serialize)]
        struct OldOp {
            origin: DeviceId,
            seq: u64,
            hlc: u64,
            body: OldBody,
        }
        let old = OldOp {
            origin: "d".into(),
            seq: 1,
            hlc: 2,
            body: OldBody::ClipAdded {
                clip_id: "c".into(),
                kind: ClipKind::Text,
                group_key: blake3_hash(b""),
                preview: String::new(),
                formats: vec![],
            },
        };
        let op = Op::decode(&rmp_serde::to_vec_named(&old).unwrap()).unwrap();
        assert!(matches!(
            op.body,
            OpBody::ClipAdded {
                source_app: None,
                trimmed: false,
                ..
            }
        ));
    }

    #[test]
    fn stub_clears_content() {
        let stub = sample_ops()[0].to_stub();
        assert!(stub.is_stub());
        let OpBody::ClipAdded {
            preview,
            formats,
            source_app,
            ..
        } = &stub.body
        else {
            panic!()
        };
        assert!(preview.is_empty() && formats.is_empty() && source_app.is_none());
    }

    #[test]
    fn ids_are_uuid_v7() {
        let id = ClipId::new_v7(1_700_000_000_123);
        let u = uuid::Uuid::parse_str(id.as_str()).unwrap();
        assert_eq!(u.get_version_num(), 7);
    }
}

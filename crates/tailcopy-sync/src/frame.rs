//! Wire frames (SPEC 7.3): one MessagePack-encoded [`Frame`] per binary WebSocket
//! message, with field names on the wire so new `#[serde(default)]` fields stay
//! compatible.

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use tailcopy_core::{DeviceId, DeviceName, Hash32, Op, Role, VersionVector};

/// Protocol version. A different major ends the session (SPEC 7.4).
pub const PROTO_MAJOR: u16 = 1;
pub const PROTO_MINOR: u16 = 0;

/// Largest frame accepted or sent.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Most ops in one `Ops` frame.
pub const MAX_OPS_PER_FRAME: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    ProtoMismatch,
    Unauthorized,
    FrameTooLarge,
    BadHash,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Hello {
        proto_major: u16,
        proto_minor: u16,
        device_id: DeviceId,
        role: Role,
        display_name: String,
    },
    Vector {
        vv: VersionVector,
    },
    Devices {
        devices: Vec<DeviceName>,
    },
    /// An inline blob, sent before the `Ops` frame that needs it.
    Blob {
        hash: Hash32,
        data: ByteBuf,
    },
    Ops {
        ops: Vec<Op>,
    },
    /// End of the backlog for this session.
    CaughtUp,
    Ping {
        t: u64,
    },
    Pong {
        t: u64,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame of {0} bytes is over the 8 MiB limit")]
    TooLarge(usize),
    #[error("couldn't encode a frame: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
    #[error("couldn't decode a frame: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
}

impl Frame {
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        let bytes = rmp_serde::to_vec_named(self)?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge(bytes.len()));
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Frame, FrameError> {
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge(bytes.len()));
        }
        Ok(rmp_serde::from_slice(bytes)?)
    }

    /// Name for logs.
    pub fn kind(&self) -> &'static str {
        match self {
            Frame::Hello { .. } => "Hello",
            Frame::Vector { .. } => "Vector",
            Frame::Devices { .. } => "Devices",
            Frame::Blob { .. } => "Blob",
            Frame::Ops { .. } => "Ops",
            Frame::CaughtUp => "CaughtUp",
            Frame::Ping { .. } => "Ping",
            Frame::Pong { .. } => "Pong",
            Frame::Error { .. } => "Error",
        }
    }
}

/// `GET /v1/hello` body (SPEC 7.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloInfo {
    pub proto_major: u16,
    pub proto_minor: u16,
    pub device_id: DeviceId,
    pub display_name: String,
    /// `device` or `relay`.
    pub role: String,
    pub app_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tailcopy_core::model::{FormatRef, blake3_hash, mime};
    use tailcopy_core::{ClipKind, OpBody};

    fn every_frame() -> Vec<Frame> {
        let id = DeviceId::from("019a0000-0000-7000-8000-000000000001");
        let op = Op {
            origin: id.clone(),
            seq: 1,
            hlc: 7,
            body: OpBody::ClipAdded {
                clip_id: "c".into(),
                kind: ClipKind::Text,
                group_key: blake3_hash(b"x"),
                preview: "x".into(),
                formats: vec![FormatRef {
                    mime: mime::TEXT.into(),
                    blob_hash: blake3_hash(b"x"),
                    size: 1,
                }],
                source_app: None,
                trimmed: false,
            },
        };
        vec![
            Frame::Hello {
                proto_major: PROTO_MAJOR,
                proto_minor: PROTO_MINOR,
                device_id: id.clone(),
                role: Role::Relay,
                display_name: "desk".into(),
            },
            Frame::Vector {
                vv: VersionVector::from([(id.clone(), 4)]),
            },
            Frame::Devices {
                devices: vec![DeviceName {
                    device_id: id.clone(),
                    display_name: "desk".into(),
                    role: Role::Device,
                    updated_hlc: 9,
                }],
            },
            Frame::Blob {
                hash: blake3_hash(b"x"),
                data: ByteBuf::from(b"x".to_vec()),
            },
            Frame::Ops { ops: vec![op] },
            Frame::CaughtUp,
            Frame::Ping { t: 1 },
            Frame::Pong { t: 1 },
            Frame::Error {
                code: ErrorCode::BadHash,
                message: "m".into(),
            },
        ]
    }

    #[test]
    fn every_variant_round_trips() {
        for frame in every_frame() {
            let bytes = frame.encode().unwrap();
            assert_eq!(Frame::decode(&bytes).unwrap(), frame, "{}", frame.kind());
        }
    }

    #[test]
    fn blob_bytes_travel_as_msgpack_bin() {
        let frame = Frame::Blob {
            hash: blake3_hash(b"x"),
            data: ByteBuf::from(vec![7u8; 300]),
        };
        let bytes = frame.encode().unwrap();
        // bin16 header (0xc5) with length 300, not an array of integers.
        assert!(bytes.windows(3).any(|w| w == [0xc5, 0x01, 0x2c]));
        assert!(bytes.len() < 400);
    }

    #[test]
    fn oversized_frames_are_refused_both_ways() {
        let frame = Frame::Blob {
            hash: blake3_hash(b"x"),
            data: ByteBuf::from(vec![0u8; MAX_FRAME_BYTES + 1]),
        };
        assert!(matches!(frame.encode(), Err(FrameError::TooLarge(_))));
        assert!(matches!(
            Frame::decode(&vec![0u8; MAX_FRAME_BYTES + 1]),
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn a_field_added_later_with_a_default_still_decodes() {
        // An older peer's Ops frame: HistoryCleared without seen and spared.
        #[derive(Serialize)]
        enum OldBody {
            HistoryCleared {
                before_hlc: u64,
                include_pinned: bool,
            },
        }
        #[derive(Serialize)]
        struct OldOp {
            origin: DeviceId,
            seq: u64,
            hlc: u64,
            body: OldBody,
        }
        #[derive(Serialize)]
        enum OldFrame {
            Ops { ops: Vec<OldOp> },
        }
        let bytes = rmp_serde::to_vec_named(&OldFrame::Ops {
            ops: vec![OldOp {
                origin: "d".into(),
                seq: 1,
                hlc: 2,
                body: OldBody::HistoryCleared {
                    before_hlc: 2,
                    include_pinned: false,
                },
            }],
        })
        .unwrap();
        let Frame::Ops { ops } = Frame::decode(&bytes).unwrap() else {
            panic!()
        };
        assert!(matches!(
            &ops[0].body,
            OpBody::HistoryCleared { seen: None, spared, .. } if spared.is_empty()
        ));
    }
}

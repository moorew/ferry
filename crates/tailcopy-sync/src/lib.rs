//! Protocol frames, peer manager, sessions, the `Transport` trait and blob fetch
//! (SPEC 7).

mod blobs;
mod dial;
mod engine;
pub mod frame;
pub mod identity;
pub mod paired;
mod server;
mod session;
pub mod store;
pub mod transport;
mod ws;

pub use engine::{Engine, EngineConfig, PeerSnapshot, SyncEvent, SyncSnapshot, TailnetState};
pub use frame::{DirEntry, ErrorCode, Frame, HelloInfo, PROTO_MAJOR, PROTO_MINOR};
pub use identity::{Identity, PairLink, PairLinkError};
pub use paired::{PairedTransport, is_tailscale_ip};
pub use store::{StoreHandle, StoreJob, StoreRunner, spawn_store_thread};
pub use transport::{Candidate, PeerIdentity, Pinned, Transport};

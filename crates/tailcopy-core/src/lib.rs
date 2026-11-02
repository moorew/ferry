//! Names, data model, HLC, ops, SQLite store, blobs, retention and search.
//!
//! The local data layer of TailCopy. No networking: the sync crate moves [`Op`]s and
//! blobs between [`Store`]s.

pub mod capture;
pub mod config;
pub mod error;
pub mod hlc;
pub mod merge;
pub mod model;
pub mod names;
pub mod search;
pub mod store;

pub use capture::{Capture, CapturedFormats, SizeLimits};
pub use config::{Config, ConfigFile, Platform, PrivacyDecision, PrivacyMode, PrivacyRule};
pub use error::{Error, Result};
pub use hlc::{Clock, ManualClock, SystemClock};
pub use model::{
    ClipId, ClipKind, DeviceId, DeviceInfo, FormatRef, Hash32, Op, OpBody, Role, VersionVector,
};
pub use search::{SearchContext, SearchHit, SearchIndex, parse_query};
pub use store::{
    ApplyReport, CaptureOutcome, ClipDetail, DisplayRow, ReadView, Reader, RetentionPolicy, Store,
    StoreOptions,
};

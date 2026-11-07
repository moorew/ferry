//! Names, data model, HLC, ops, SQLite store, blobs, retention and search.
//!
//! The local data layer of TailCopy. No networking: the sync crate moves [`Op`]s and
//! blobs between [`Store`]s.

pub mod capture;
pub mod config;
pub mod error;
pub mod hlc;
pub mod look;
pub mod merge;
pub mod model;
pub mod names;
pub mod search;
pub mod store;

pub use capture::{Capture, CapturedFormats, SizeLimits};
pub use config::{Config, ConfigFile, Platform, PrivacyDecision, PrivacyMode, PrivacyRule, Theme};
pub use error::{Error, Result};
pub use hlc::{Clock, ManualClock, SystemClock};
pub use look::Look;
pub use model::{
    ClipId, ClipKind, DeviceId, DeviceInfo, DeviceName, FormatRef, Hash32, Op, OpBody, PeerKey,
    PeerSeen, Role, SyncFlags, VersionVector,
};
pub use search::{Counts, SearchContext, SearchHit, SearchIndex, parse_query};
pub use store::{
    ApplyReport, CaptureOutcome, ClipDetail, DisplayRow, ReadView, Reader, RetentionPolicy, Store,
    StoreOptions,
};

#[cfg(test)]
mod tests {
    fn assert_send<T: Send>() {}

    /// The app moves these into an actor thread.
    #[test]
    fn store_types_are_send() {
        assert_send::<super::Store>();
        assert_send::<super::Reader>();
        assert_send::<super::SearchIndex>();
        assert_send::<super::ConfigFile>();
    }
}

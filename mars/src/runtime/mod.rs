/** Composition root owns engine lifetimes, committed pages and native tools. */
mod agent;
mod animations;
pub mod authority;
mod batch;
/** Bookmarks the person saves; desktop-owned, persisted through the profile record store. */
pub mod bookmarks;
mod broadcast;
mod cancellation;
mod csp;
mod demo;
/** Native operations behind the CDP front door: evaluation, the DOM view, cookies, emulation and
request interception, each under the same ownership rules as every other agent operation. */
mod devtools;
/** Saving files fetched through the governed network path. */
pub mod downloads;
mod exif;
mod frame_pages;
mod frame_router;
mod frame_tree;
mod frames;
mod framing;
/** Committed pages the person visited and address-field completion. */
pub mod history;
mod horizontal;
mod images;
mod indexed_db;
mod integrity;
mod interaction;
mod intersections;
mod languages;
mod lifetime;
/** The Oya server link's way into the runtime, under the same ownership checks as local agents. */
pub mod link;
mod loader;
pub mod media;
mod modules;
mod names;
mod navigation;
mod observe;
mod page;
/** Applies an Oya persona to a runtime as one immutable unit. */
pub mod persona;
mod platform;
/** Size bounds of the sealed records the platform APIs persist. */
pub mod platform_records {
    /** Service worker registrations with their scripts. */
    pub const SERVICE_WORKERS_BYTES: usize = 16 * 1024 * 1024;
    /** Cache Storage entries with their bodies. */
    pub const CACHE_STORAGE_BYTES: usize = 32 * 1024 * 1024;
    /** Stored password and federated credentials. */
    pub const CREDENTIALS_BYTES: usize = 1024 * 1024;
}
mod presentation;
mod residency;
mod resize_observers;
mod resources;
/** Per-engine crash restart (CB-23). */
mod restart;
mod same_document;
mod scripting;
mod service;
/** The person's browser settings. */
pub mod settings;
mod supervisor;
/** Selecting and copying painted page text. */
pub mod text_selection;
mod worker_io;
mod worker_loading;
mod workers;
/** Standard padded base64; desktop captures return PNGs inline with it. */
pub(crate) use agent::base64;
pub use agent::mutating;
pub use agent::run_agent;
pub use demo::run_demo;
pub use observe::WAIT_MAX_MS;
pub use page::BrowserSnapshot;
pub use platform::DecryptError;
pub use service::run_service;
/// Opt-in phase timing (MARS_PERF), so the desktop and tests can log what they do with a snapshot.
pub use supervisor::perf;
pub use supervisor::tab_key;
pub use supervisor::{EnginePrograms, Runtime};

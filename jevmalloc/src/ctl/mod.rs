//! Opinionated control and introspection for jemalloc.
//!
//! Every built-in control keeps a process-wide cache after successful MIB
//! translation, then calls `mallctlbymib` directly. [`Arena`] represents one
//! explicitly created or borrowed instance, while [`arenas`] contains allocator
//! defaults and the controls that intentionally select all arenas.
//!
//! The safe functions encode each selected control's C value type. The
//! [`raw`] module remains available for controls outside this curated surface,
//! but its generic operations are unsafe because a MIB carries neither type
//! information nor a command's semantic preconditions.

mod arena;
mod error;
mod key;
#[cfg(feature = "profiling")]
mod profiling;
mod stats;
mod value;

pub mod arenas;
pub mod raw;
pub mod this_thread;

#[cfg(feature = "profiling")]
pub use self::profiling::{
	is_prof_enabled, prof_dump, prof_enable, prof_gdump, prof_interval, prof_reset,
};
#[cfg(feature = "stats")]
pub use self::stats::stats_reset;
pub use self::{
	arena::{
		ARENA_INDEX_LIMIT, ARENA_NAME_LEN, Arena, ArenaDestroyError, ArenaName, Dss, ExtentHooks,
	},
	error::Error,
	key::{KEY_SEGS, Key, NAME_MAX},
	stats::{epoch, refresh_epoch},
};
use super::std;

/// The result of a jemalloc control operation.
///
/// A failed operation retains the nonzero status returned by jemalloc. Invalid
/// names supplied to [`raw::mibs`] are reported as `EINVAL`.
pub type Result<T = ()> = std::result::Result<T, Error>;

/// Enables or disables jemalloc's background purge workers.
///
/// Enabling creates workers on demand. Disabling waits for existing workers to
/// terminate before returning. The previous setting is returned. If jemalloc
/// reports a worker-management failure, the requested setting can still have
/// taken effect. A child process starts with workers disabled after `fork`,
/// regardless of the parent setting. The control exists only on selected
/// pthread-based platforms.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the read or update.
pub fn background_thread_enable(enable: bool) -> Result<bool> {
	let key = key::background_thread()?;
	value::xchg_bool(&key, enable)
}

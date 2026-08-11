//! Opinionated control and introspection for jemalloc.
//!
//! This module exposes the small set of allocator controls used by Tuwunel.
//! Every built-in control keeps a process-wide cache after successful MIB
//! translation, then calls `mallctlbymib` directly. Arena controls share one
//! implementation: [`this_thread`] substitutes the calling thread's arena,
//! explicit arena operations substitute the requested identifier, and `None`
//! selects all arenas where jemalloc supports that convention.
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
		arenas, decay, is_affine_arena, is_percpu_arena, is_phycpu_arena, percpu_arenas, purge,
		quantum, set_dirty_decay, set_muzzy_decay, trim,
	},
	error::Error,
	stats::{epoch, refresh_epoch},
};
use super::std;

/// The result of a jemalloc control operation.
///
/// A failed operation retains the nonzero status returned by jemalloc. Invalid
/// names supplied to [`raw::mibs`] are reported as `EINVAL`.
pub type Result<T = ()> = std::result::Result<T, Error>;

/// A resolved jemalloc Management Information Base key.
///
/// Jemalloc currently uses at most seven numeric components. The extra slot
/// leaves room for compatible namespace growth while keeping every key inline.
pub type Key = arrayvec::ArrayVec<usize, KEY_SEGS>;

/// Maximum number of bytes in a control name, including its trailing NUL.
const NAME_MAX: usize = 128;

/// Number of inline numeric components reserved for a MIB key.
const KEY_SEGS: usize = 8;

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

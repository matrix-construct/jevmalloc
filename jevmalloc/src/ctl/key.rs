//! Process-wide caching for built-in MIB keys.

/// Caches one fixed control name as an inline MIB.
mod cache;

#[cfg(test)]
mod tests;

use self::cache::Cache;
use super::{Error, Result, raw};

/// A resolved jemalloc Management Information Base key.
///
/// Jemalloc currently uses at most seven numeric components. The extra slot
/// leaves room for compatible namespace growth while keeping every key inline.
pub type Key = arrayvec::ArrayVec<usize, KEY_SEGS>;

/// Maximum number of bytes in a control name, including its trailing NUL.
pub const NAME_MAX: usize = 128;

/// Number of inline numeric components reserved for a MIB key.
pub const KEY_SEGS: usize = 8;

/// Defines one process-wide cache and accessor for a fixed control name.
macro_rules! define_key {
	($accessor:ident, $name:literal) => {
		#[doc = concat!("Returns the cached MIB for `", $name, "`.")]
		pub(super) fn $accessor() -> Result<Key> {
			static KEY: Cache = Cache::new();
			KEY.get($name)
		}
	};
}

define_key!(epoch, "epoch");
define_key!(background_thread, "background_thread");
define_key!(opt_percpu_arena, "opt.percpu_arena");
define_key!(arena_purge, "arena.4096.purge");
define_key!(arena_decay, "arena.4096.decay");
define_key!(arena_muzzy_decay, "arena.4096.muzzy_decay_ms");
define_key!(arena_dirty_decay, "arena.4096.dirty_decay_ms");
define_key!(arenas_muzzy_decay, "arenas.muzzy_decay_ms");
define_key!(arenas_dirty_decay, "arenas.dirty_decay_ms");
define_key!(arenas_count, "arenas.narenas");
define_key!(arenas_quantum, "arenas.quantum");
define_key!(thread_idle, "thread.idle");
define_key!(thread_arena, "thread.arena");
define_key!(thread_tcache_flush, "thread.tcache.flush");
define_key!(thread_arena_purge, "arena.0.purge");
define_key!(thread_arena_decay, "arena.0.decay");
define_key!(thread_muzzy_decay, "arena.0.muzzy_decay_ms");
define_key!(thread_dirty_decay, "arena.0.dirty_decay_ms");
define_key!(thread_tcache_enabled, "thread.tcache.enabled");

#[cfg(feature = "profiling")]
define_key!(prof_reset, "prof.reset");
#[cfg(feature = "profiling")]
define_key!(prof_dump, "prof.dump");
#[cfg(feature = "profiling")]
define_key!(prof_gdump, "prof.gdump");
#[cfg(feature = "profiling")]
define_key!(prof_active, "prof.active");
#[cfg(feature = "profiling")]
define_key!(prof_interval, "prof.interval");
#[cfg(feature = "profiling")]
define_key!(thread_prof_active, "thread.prof.active");

#[cfg(feature = "stats")]
define_key!(stats_mutexes_reset, "stats.mutexes.reset");
#[cfg(feature = "stats")]
define_key!(thread_peak_reset, "thread.peak.reset");
#[cfg(feature = "stats")]
define_key!(thread_peak_read, "thread.peak.read");
#[cfg(feature = "stats")]
define_key!(thread_allocated, "thread.allocated");
#[cfg(feature = "stats")]
define_key!(thread_deallocated, "thread.deallocated");
#[cfg(feature = "stats")]
define_key!(thread_allocatedp, "thread.allocatedp");
#[cfg(feature = "stats")]
define_key!(thread_deallocatedp, "thread.deallocatedp");

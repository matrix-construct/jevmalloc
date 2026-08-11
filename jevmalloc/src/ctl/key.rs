//! Process-wide caching for built-in MIB keys.

use core::sync::atomic::{AtomicUsize, Ordering};

use super::{KEY_SEGS, Key, Result, raw};

/// Caches one fixed control name as an inline MIB.
///
/// A zero length means unpublished. Initializers write every segment before a
/// release store publishes the nonzero length. Concurrent first callers may
/// translate the same immutable name more than once, but all segment writes are
/// atomic and identical, so no caller blocks or observes a partial key.
pub(super) struct Cache {
	/// Published key length, or zero before successful translation.
	len: AtomicUsize,

	/// Numeric MIB components.
	segments: [AtomicUsize; KEY_SEGS],
}

impl Cache {
	/// Constructs an empty cache for a static control name.
	pub(super) const fn new() -> Self {
		Self {
			len: AtomicUsize::new(0),
			segments: [const { AtomicUsize::new(0) }; KEY_SEGS],
		}
	}

	/// Returns the cached MIB, translating `name` on the first successful call.
	pub(super) fn get(&self, name: &str) -> Result<Key> {
		let len = self.len.load(Ordering::Acquire);
		if len > 0 {
			return self.load(len);
		}

		let key = raw::mibs(name)?;
		for (slot, segment) in self.segments.iter().zip(key.iter().copied()) {
			slot.store(segment, Ordering::Relaxed);
		}

		self.len.store(key.len(), Ordering::Release);
		Ok(key)
	}

	/// Copies a published MIB out of the atomic storage.
	fn load(&self, len: usize) -> Result<Key> {
		let mut key = Key::new();
		for segment in self.segments.iter().take(len) {
			key.try_push(segment.load(Ordering::Relaxed))
				.map_err(|_| super::Error::invalid_argument())?;
		}

		Ok(key)
	}
}

/// Defines one process-wide cache and accessor for a fixed control name.
macro_rules! define_key {
	($accessor:ident, $name:literal) => {
		#[inline]
		#[doc = concat!("Returns the cached MIB for `", $name, "`.")]
		pub(super) fn $accessor() -> Result<Key> {
			static KEY: Cache = Cache::new();
			KEY.get($name)
		}
	};
}

define_key!(background_thread, "background_thread");
define_key!(arena_purge, "arena.4096.purge");
define_key!(arena_decay, "arena.4096.decay");
define_key!(arena_muzzy_decay, "arena.4096.muzzy_decay_ms");
define_key!(arena_dirty_decay, "arena.4096.dirty_decay_ms");
define_key!(arenas_muzzy_decay, "arenas.muzzy_decay_ms");
define_key!(arenas_dirty_decay, "arenas.dirty_decay_ms");
define_key!(arenas_count, "arenas.narenas");
define_key!(arenas_quantum, "arenas.quantum");
define_key!(percpu_arena, "opt.percpu_arena");
define_key!(epoch, "epoch");

#[cfg(feature = "stats")]
define_key!(stats_mutexes_reset, "stats.mutexes.reset");

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

define_key!(thread_idle, "thread.idle");
define_key!(thread_tcache_flush, "thread.tcache.flush");
define_key!(thread_arena_purge, "arena.0.purge");
define_key!(thread_arena_decay, "arena.0.decay");
define_key!(thread_muzzy_decay, "arena.0.muzzy_decay_ms");
define_key!(thread_dirty_decay, "arena.0.dirty_decay_ms");
define_key!(thread_tcache_enabled, "thread.tcache.enabled");
define_key!(thread_arena, "thread.arena");

#[cfg(feature = "profiling")]
define_key!(thread_prof_active, "thread.prof.active");

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

#[cfg(test)]
mod tests {
	//! Checks publication and reuse of a cached key.

	extern crate std as rust_std;

	use rust_std::{sync::Barrier, thread};

	use super::*;

	/// Confirms that the published MIB matches a fresh translation.
	#[test]
	fn cached_key_matches_translation() {
		static CACHE: Cache = Cache::new();

		let first = CACHE.get("epoch").unwrap();
		let second = CACHE.get("epoch").unwrap();

		assert_eq!(first, raw::mibs("epoch").unwrap());
		assert_eq!(second, first);
	}

	/// Exercises concurrent first publication of one immutable key.
	#[test]
	fn publishes_concurrently() {
		static CACHE: Cache = Cache::new();
		static START: Barrier = Barrier::new(4);

		thread::scope(|scope| {
			for _ in 0..4 {
				scope.spawn(|| {
					START.wait();
					assert_eq!(CACHE.get("epoch").unwrap(), raw::mibs("epoch").unwrap());
				});
			}
		});
	}
}

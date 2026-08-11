use core::sync::atomic::{AtomicUsize, Ordering};

use super::{Error, KEY_SEGS, Key, Result, raw};

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
				.map_err(|_| Error::invalid_argument())?;
		}

		Ok(key)
	}
}

//! Stable-address extent hook tables for explicit arenas.

use crate::ffi::{
	self, extent_alloc_t, extent_commit_t, extent_dalloc_t, extent_decommit_t, extent_destroy_t,
	extent_merge_t, extent_purge_t, extent_split_t,
};

/// A complete custom extent hook table with a mandatory allocation callback.
///
/// Build the table in static storage before installing it in an arena. Jemalloc
/// reads it directly and may invoke its callbacks concurrently until successful
/// arena destruction.
#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct ExtentHooks(ffi::extent_hooks_t);

impl ExtentHooks {
	/// Constructs a table with only the mandatory allocation hook.
	///
	/// Every other operation initially opts out. Builder methods configure the
	/// remaining callbacks before the value is placed in static storage and
	/// installed.
	#[must_use]
	pub const fn new(alloc: extent_alloc_t) -> Self {
		Self(ffi::extent_hooks_t {
			alloc: Some(alloc),
			dalloc: None,
			destroy: None,
			commit: None,
			decommit: None,
			purge_lazy: None,
			purge_forced: None,
			split: None,
			merge: None,
		})
	}

	/// Installs the extent deallocation callback.
	///
	/// Returning failure from this callback retains the mapping for later
	/// reuse.
	#[must_use]
	pub const fn with_deallocate(mut self, hook: extent_dalloc_t) -> Self {
		self.0.dalloc = Some(hook);
		self
	}

	/// Installs the unconditional extent destruction callback.
	///
	/// Arena destruction can invoke this callback for retained extents.
	#[must_use]
	pub const fn with_destroy(mut self, hook: extent_destroy_t) -> Self {
		self.0.destroy = Some(hook);
		self
	}

	/// Installs the physical memory commit callback.
	///
	/// Successful commits must provide zeroed pages for the requested range.
	#[must_use]
	pub const fn with_commit(mut self, hook: extent_commit_t) -> Self {
		self.0.commit = Some(hook);
		self
	}

	/// Installs the physical memory decommit callback.
	///
	/// Returning failure leaves the requested range committed.
	#[must_use]
	pub const fn with_decommit(mut self, hook: extent_decommit_t) -> Self {
		self.0.decommit = Some(hook);
		self
	}

	/// Installs the lazy page purge callback.
	///
	/// A successful lazy purge may leave page contents indeterminate until the
	/// operating system performs the purge.
	#[must_use]
	pub const fn with_lazy_purge(mut self, hook: extent_purge_t) -> Self {
		self.0.purge_lazy = Some(hook);
		self
	}

	/// Installs the forced page purge callback.
	///
	/// A successful forced purge must make the pages read as zero on reuse.
	#[must_use]
	pub const fn with_forced_purge(mut self, hook: extent_purge_t) -> Self {
		self.0.purge_forced = Some(hook);
		self
	}

	/// Installs the extent split callback.
	///
	/// Returning failure leaves the original extent whole.
	#[must_use]
	pub const fn with_split(mut self, hook: extent_split_t) -> Self {
		self.0.split = Some(hook);
		self
	}

	/// Installs the adjacent extent merge callback.
	///
	/// Returning failure leaves the extents as distinct mappings.
	#[must_use]
	pub const fn with_merge(mut self, hook: extent_merge_t) -> Self {
		self.0.merge = Some(hook);
		self
	}

	/// Returns the underlying C-compatible hook table.
	///
	/// The shared reference does not permit callback replacement after the
	/// table has been installed.
	#[must_use]
	pub const fn as_raw(&self) -> &ffi::extent_hooks_t { &self.0 }
}

#[cfg(test)]
mod tests {
	//! Checks that the builder can populate every ABI callback slot.

	use core::ptr::null_mut;

	#[cfg(target_env = "msvc")]
	use libc::c_int;
	use libc::{c_uint, c_void, size_t};

	use super::*;

	/// Jemalloc's C boolean representation under cl.exe.
	#[cfg(target_env = "msvc")]
	type CBool = c_int;

	/// Jemalloc's C boolean representation on non-MSVC targets.
	#[cfg(not(target_env = "msvc"))]
	type CBool = bool;

	/// Returns the callback representation for failure or opt-out.
	#[cfg(target_env = "msvc")]
	const fn failure() -> CBool { 1 }

	/// Returns the callback representation for failure or opt-out.
	#[cfg(not(target_env = "msvc"))]
	const fn failure() -> CBool { true }

	/// Test allocation callback that always reports failure.
	unsafe extern "C" fn allocate(
		_hooks: *mut ffi::extent_hooks_t,
		_new_addr: *mut c_void,
		_size: size_t,
		_alignment: size_t,
		_zero: *mut CBool,
		_commit: *mut CBool,
		_arena: c_uint,
	) -> *mut c_void {
		null_mut()
	}

	/// Test deallocation callback that always opts out.
	unsafe extern "C" fn deallocate(
		_hooks: *mut ffi::extent_hooks_t,
		_addr: *mut c_void,
		_size: size_t,
		_committed: CBool,
		_arena: c_uint,
	) -> CBool {
		failure()
	}

	/// Test destruction callback with no backing mapping.
	unsafe extern "C" fn destroy(
		_hooks: *mut ffi::extent_hooks_t,
		_addr: *mut c_void,
		_size: size_t,
		_committed: CBool,
		_arena: c_uint,
	) {
	}

	/// Test range callback that always opts out.
	unsafe extern "C" fn range(
		_hooks: *mut ffi::extent_hooks_t,
		_addr: *mut c_void,
		_size: size_t,
		_offset: size_t,
		_length: size_t,
		_arena: c_uint,
	) -> CBool {
		failure()
	}

	/// Test split callback that always opts out.
	unsafe extern "C" fn split(
		_hooks: *mut ffi::extent_hooks_t,
		_addr: *mut c_void,
		_size: size_t,
		_size_a: size_t,
		_size_b: size_t,
		_committed: CBool,
		_arena: c_uint,
	) -> CBool {
		failure()
	}

	/// Test merge callback that always opts out.
	unsafe extern "C" fn merge(
		_hooks: *mut ffi::extent_hooks_t,
		_addr_a: *mut c_void,
		_size_a: size_t,
		_addr_b: *mut c_void,
		_size_b: size_t,
		_committed: CBool,
		_arena: c_uint,
	) -> CBool {
		failure()
	}

	/// Populates all nine callback slots through the typed builder.
	#[test]
	fn populates_every_callback_slot() {
		let hooks = ExtentHooks::new(allocate)
			.with_deallocate(deallocate)
			.with_destroy(destroy)
			.with_commit(range)
			.with_decommit(range)
			.with_lazy_purge(range)
			.with_forced_purge(range)
			.with_split(split)
			.with_merge(merge);
		let raw = hooks.as_raw();

		assert!(raw.alloc.is_some());
		assert!(raw.dalloc.is_some());
		assert!(raw.destroy.is_some());
		assert!(raw.commit.is_some());
		assert!(raw.decommit.is_some());
		assert!(raw.purge_lazy.is_some());
		assert!(raw.purge_forced.is_some());
		assert!(raw.split.is_some());
		assert!(raw.merge.is_some());
	}

	/// Confirms that an immutable hook table can be shared across callback
	/// threads.
	#[test]
	fn table_is_send_and_sync() {
		fn assert_send_sync<T: Send + Sync>() {}

		assert_send_sync::<ExtentHooks>();
	}
}

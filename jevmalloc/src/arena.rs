//! One jemalloc arena and its explicit instance lifecycle.
//!
//! Statistics-facing controls, including `arena.<i>.initialized`, remain out
//! of scope with the rest of the arena statistics interface.

mod destroy_error;
mod dss;
mod extent_hooks;
mod name;

#[cfg(test)]
mod tests;

use core::{
	ffi::{CStr, c_void},
	ptr::NonNull,
	result,
};

use libc::{c_char, c_int, c_uint};

pub use self::{
	destroy_error::ArenaDestroyError,
	dss::Dss,
	extent_hooks::{
		EMPTY_RAW_EXTENT_HOOKS, Extent, ExtentAlloc, ExtentAllocFn, ExtentAllocation,
		ExtentCallbacks, ExtentDallocFn, ExtentDestroyFn, ExtentHookResult, ExtentHooks,
		ExtentMerge, ExtentMergeFn, ExtentRange, ExtentRangeFn, ExtentSplit, ExtentSplitFn,
		RawExtentHooks,
	},
	name::{ARENA_NAME_LEN, ArenaName},
};
use crate::{
	ctl::{Error, Key, Result, key, raw},
	ffi, thread,
};

/// Exclusive upper bound for ordinary jemalloc arena indices.
///
/// Jemalloc reserves 12 bits for arena selection and one encoded value. Valid
/// ordinary indices are therefore in `0..4095`; `4096` denotes all arenas for
/// the small set of controls that support that sentinel.
pub const ARENA_INDEX_LIMIT: usize = 4095;

/// An owning or non-owning handle to one jemalloc arena.
///
/// [`Arena::create`] returns an owner that attempts `arena.<i>.destroy` when
/// dropped. Handles obtained from an index, the current thread, or an
/// allocation are non-owning and never destroy the arena.
#[must_use = "an owned arena is destroyed when its handle is dropped"]
#[derive(Debug)]
pub struct Arena {
	/// Numeric arena component used in MIB keys and allocation flags.
	index: usize,

	/// Whether this handle owns destruction of an explicitly created arena.
	owned: bool,
}

impl Arena {
	/// Creates an explicitly managed arena with jemalloc's default extent
	/// hooks.
	///
	/// The returned owner destroys an otherwise unused arena when dropped. Any
	/// operation that directs allocations into it is unsafe because those
	/// allocations and their caches must not outlive the owner.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc cannot create the arena or returns an
	/// invalid index.
	pub fn create() -> Result<Self> {
		let key = key::arenas_create()?;

		// SAFETY: `arenas.create` with no input creates one default-hook arena and
		// writes its `unsigned` index. The new owner records that lifecycle.
		let index = unsafe { raw::get::<c_uint>(&key) }?;

		Self::from_created_index(index)
	}

	/// Creates an explicitly managed arena with a custom extent hook table.
	///
	/// The table is used for arena metadata during this call and remains in use
	/// until successful destruction. All callbacks except allocation may opt
	/// out. A custom table prevents jemalloc's Huge Page Allocator from being
	/// enabled for this arena.
	///
	/// # Errors
	///
	/// Returns an error if the table has no allocation callback, jemalloc
	/// cannot create the arena, or jemalloc returns an invalid index.
	///
	/// # Safety
	///
	/// Every callback must uphold its mapping contract and never unwind into
	/// jemalloc. Any mutable callback state must synchronize concurrent calls.
	pub unsafe fn create_with_extent_hooks<State: Sync + 'static>(
		hooks: &'static ExtentHooks<State>,
	) -> Result<Self> {
		let hooks = hooks.as_raw();

		// SAFETY: the static stateful table preserves the raw header's address. The
		// caller guarantees every callback contract.
		unsafe { Self::create_with_raw_extent_hooks(hooks) }
	}

	/// Creates an explicitly managed arena from a raw extent hook table
	/// pointer.
	///
	/// This is the escape hatch for foreign hook tables and jemalloc's own
	/// default table. Prefer [`Arena::create_with_extent_hooks`] for
	/// Rust-defined tables.
	///
	/// # Errors
	///
	/// Returns an error if the pointer has no allocation hook, jemalloc cannot
	/// create the arena, or jemalloc returns an invalid index.
	///
	/// # Safety
	///
	/// `hooks` must remain valid and immutable for the rest of the process. Its
	/// callbacks and reachable state must satisfy the same requirements as
	/// [`Arena::create_with_extent_hooks`].
	pub unsafe fn create_with_raw_extent_hooks(hooks: NonNull<RawExtentHooks>) -> Result<Self> {
		// SAFETY: the caller guarantees that `hooks` points to a live table.
		if unsafe { hooks.as_ref() }.alloc.is_none() {
			return Err(Error::invalid_argument());
		}

		let key = key::arenas_create()?;
		let hooks = hooks.as_ptr();

		// SAFETY: `arenas.create` accepts an `extent_hooks_t *` input and writes
		// an `unsigned` index. The pointer satisfies its process-lifetime contract.
		let index = unsafe { raw::update::<_, c_uint>(&key, &hooks) }?;

		Self::from_created_index(index)
	}

	/// Constructs a non-owning handle from a numeric arena index.
	///
	/// This validates only the representable index range. It does not
	/// initialize the arena, prove that it is live, or protect against index
	/// recycling.
	///
	/// # Errors
	///
	/// Returns an error if `index` is outside jemalloc's ordinary arena range
	/// or does not fit its C `unsigned` representation.
	///
	/// # Safety
	///
	/// The index must remain live and unrecycled for every use of the returned
	/// handle. The caller must synchronize its operations with any owner that
	/// can reset or destroy the arena.
	pub unsafe fn from_index(index: usize) -> Result<Self> {
		validated_index(index)?;

		Ok(Self { index, owned: false })
	}

	/// Reconstructs ownership of an explicitly created arena index.
	///
	/// This supports ownership transfer through [`Arena::into_index`]. The
	/// returned handle attempts destruction when dropped.
	///
	/// # Errors
	///
	/// Returns an error if `index` is outside jemalloc's ordinary arena range
	/// or does not fit its C `unsigned` representation.
	///
	/// # Safety
	///
	/// The index must name a live arena created through `arenas.create`, and no
	/// other owner may destroy it. All allocations, caches, associations, and
	/// operations must satisfy [`Arena::try_destroy`]'s contract. Every custom
	/// hook table installed during the arena's history must remain valid and
	/// immutable for the rest of the process because fallback destruction can
	/// fail without reporting the error.
	pub unsafe fn from_owned_index(index: usize) -> Result<Self> {
		validated_index(index)?;

		Ok(Self { index, owned: true })
	}

	/// Returns a non-owning handle to the calling thread's associated arena.
	///
	/// The handle does not pin the arena against destruction or index
	/// recycling.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the thread query or reports an
	/// invalid arena index.
	pub fn current() -> Result<Self> {
		let index = thread::this::arena_id()?;

		// SAFETY: the current thread's association pins a manual arena against
		// destruction. Any unsafe reassignment must account for handles it unpins.
		unsafe { Self::from_index(index) }
	}

	/// Finds the arena that owns a live allocation.
	///
	/// The returned handle is non-owning and does not extend the allocation or
	/// arena lifetime.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc does not recognize the allocation, rejects
	/// the lookup, or reports an invalid arena index.
	///
	/// # Safety
	///
	/// `allocation` must point into a live allocation owned by this jemalloc
	/// instance. The allocation and its arena must remain live for every use of
	/// the returned handle, synchronized with reset and destruction.
	pub unsafe fn lookup<T>(allocation: NonNull<T>) -> Result<Self> {
		let key = key::arenas_lookup()?;
		let allocation = allocation.cast::<c_void>().as_ptr();

		// SAFETY: `arenas.lookup` accepts a live `void *` allocation and writes
		// its owning arena as an `unsigned` index.
		let index = unsafe { raw::update::<_, c_uint>(&key, &allocation) }?;

		// SAFETY: the caller guarantees that the allocation keeps its arena live
		// and synchronizes the returned handle with lifecycle operations.
		unsafe {
			Self::from_index(usize::try_from(index).map_err(|_| Error::invalid_argument())?)
		}
	}

	/// Returns this arena's numeric index.
	///
	/// Destroyed indices may be recycled, so the number alone carries no
	/// ownership or liveness guarantee.
	#[must_use]
	#[inline]
	pub const fn index(&self) -> usize { self.index }

	/// Returns whether this handle owns an explicitly created arena.
	///
	/// Only an owning handle attempts destruction when dropped.
	#[must_use]
	#[inline]
	pub const fn is_owned(&self) -> bool { self.owned }

	/// Returns the extended-allocation flag selecting this arena.
	///
	/// The caller can combine it with alignment, zeroing, or tcache flags
	/// before an unsafe extended allocation call.
	#[must_use]
	#[inline(always)]
	pub const fn flags(&self) -> c_int { ffi::MALLOCX_ARENA(self.index) }

	/// Associates the calling thread with this arena.
	///
	/// Jemalloc returns the previous association as a non-owning handle. The
	/// reassignment does not flush the thread's automatic cache.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the reassignment or reports an
	/// invalid previous index.
	///
	/// # Safety
	///
	/// Every allocation made through the association must be dead before this
	/// arena is reset or destroyed. The thread must first move to another arena
	/// and flush every automatic or explicit tcache that touched this one. All
	/// handles and operations involving either association must remain
	/// synchronized with reset, destruction, and index recycling.
	pub unsafe fn set_current(&self) -> Result<Self> {
		// SAFETY: the caller accepts the allocation and cache lifetime contract.
		let previous = unsafe { thread::this::set_arena(self.index) }?;

		// SAFETY: the caller accepts responsibility for the previous arena's
		// liveness after removing the thread association that pinned it.
		unsafe { Self::from_index(previous) }
	}

	/// Relinquishes destruction and returns the numeric index.
	///
	/// The arena remains live after this call. Ownership can be recovered with
	/// [`Arena::from_owned_index`] when its safety requirements can be met.
	#[must_use]
	pub fn into_index(mut self) -> usize {
		self.owned = false;
		self.index
	}

	/// Applies decay-based purging and then purges all remaining unused dirty
	/// pages.
	///
	/// This is the instance equivalent of [`crate::arenas::trim`].
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects either reclamation command.
	pub fn trim(&self) -> Result {
		self.decay()?;
		self.purge()
	}

	/// Applies time-based dirty and muzzy page decay to this arena.
	///
	/// Jemalloc uses the arena's current dirty and muzzy decay intervals.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or decay command.
	pub fn decay(&self) -> Result { self.notify(key::arena_decay()?) }

	/// Purges all unused dirty pages from this arena.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or purge command.
	pub fn purge(&self) -> Result { self.notify(key::arena_purge()?) }

	/// Discards every extant allocation and arena-internal cached extent.
	///
	/// The arena remains initialized with the same index and settings
	/// afterward. Only explicitly created arenas can be reset.
	///
	/// # Errors
	///
	/// Returns an error if this handle is non-owning or jemalloc rejects the
	/// reset.
	///
	/// # Safety
	///
	/// No allocation from this arena may remain accessible. Every operation
	/// must be quiescent, and every automatic or explicit tcache that touched
	/// the arena must be flushed before this call.
	pub unsafe fn reset(&mut self) -> Result {
		if !self.owned {
			return Err(Error::invalid_argument());
		}

		self.notify(key::arena_reset()?)
	}

	/// Returns a copy of this arena's descriptive name.
	///
	/// The inline result contains at most 31 bytes plus its terminator.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or name query.
	pub fn name(&self) -> Result<ArenaName> {
		let key = self.select(key::arena_name()?);
		let mut name = ArenaName::default();
		let mut name_ptr = name.as_mut_ptr();

		// SAFETY: `arena.<i>.name` expects an initialized `char *` output slot
		// pointing to a writable 32-byte buffer. `ArenaName` supplies that buffer.
		unsafe { raw::get_in_place(&key, &mut name_ptr) }?;

		Ok(name)
	}

	/// Sets this arena's descriptive name from a terminated byte string.
	///
	/// Jemalloc copies the input during this call and truncates content beyond
	/// 31 bytes, so the source does not need to remain live afterward.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or name.
	pub fn set_name(&self, name: &CStr) -> Result {
		let key = self.select(key::arena_name()?);
		let name = name.as_ptr();

		// SAFETY: `arena.<i>.name` accepts a non-null `char *` input. Jemalloc
		// copies from the terminated string during this call and retains no pointer.
		unsafe { raw::set(&key, &name) }
	}

	/// Returns this arena's dynamic storage allocation precedence.
	///
	/// Custom extent allocation hooks make the returned setting irrelevant.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or reports an unknown
	/// value.
	pub fn dss(&self) -> Result<Dss> {
		let key = self.select(key::arena_dss()?);

		// SAFETY: `arena.<i>.dss` returns a process-lifetime `const char *`.
		let dss = unsafe { raw::get::<*const c_char>(&key) }?;

		// SAFETY: jemalloc returns a readable static precedence string.
		unsafe { Dss::from_ptr(dss) }
	}

	/// Sets this arena's dynamic storage allocation precedence.
	///
	/// The resulting setting is returned. Platforms without `sbrk(2)` reject
	/// settings other than [`Dss::Disabled`]. Retained DSS mappings leak during
	/// arena destruction, so destructible arenas should normally disable DSS or
	/// use custom hooks that track those mappings.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or requested setting.
	pub fn set_dss(&self, dss: Dss) -> Result<Dss> {
		let key = self.select(key::arena_dss()?);
		let dss = dss.as_ptr();

		// SAFETY: `arena.<i>.dss` accepts and returns static `const char *`
		// precedence strings. Jemalloc retains neither input pointer.
		let resulting = unsafe { raw::xchg(&key, &dss) }?;

		// SAFETY: jemalloc returns a readable static precedence string.
		unsafe { Dss::from_ptr(resulting) }
	}

	/// Returns this arena's dirty-page decay interval in milliseconds.
	///
	/// A value of `-1` disables purging and `0` requests immediate purging.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or query.
	pub fn dirty_decay(&self) -> Result<isize> {
		let key = self.select(key::arena_dirty_decay()?);

		// SAFETY: this complete MIB has C output type `ssize_t`, represented by
		// `isize` on supported targets.
		unsafe { raw::get(&key) }
	}

	/// Sets this arena's dirty-page decay interval in milliseconds.
	///
	/// The previous interval is returned. Every write fully ages unused dirty
	/// pages and can trigger immediate purging.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or interval.
	pub fn set_dirty_decay(&self, decay_ms: isize) -> Result<isize> {
		let key = self.select(key::arena_dirty_decay()?);

		// SAFETY: this complete MIB uses `ssize_t`, represented by `isize`, for
		// both input and output on supported targets.
		unsafe { raw::xchg(&key, &decay_ms) }
	}

	/// Returns this arena's muzzy-page decay interval in milliseconds.
	///
	/// A value of `-1` disables purging and `0` requests immediate purging.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or query.
	pub fn muzzy_decay(&self) -> Result<isize> {
		let key = self.select(key::arena_muzzy_decay()?);

		// SAFETY: this complete MIB has C output type `ssize_t`, represented by
		// `isize` on supported targets.
		unsafe { raw::get(&key) }
	}

	/// Sets this arena's muzzy-page decay interval in milliseconds.
	///
	/// The previous interval is returned. Every write fully ages unused muzzy
	/// pages and can trigger immediate purging.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena or interval.
	pub fn set_muzzy_decay(&self, decay_ms: isize) -> Result<isize> {
		let key = self.select(key::arena_muzzy_decay()?);

		// SAFETY: this complete MIB uses `ssize_t`, represented by `isize`, for
		// both input and output on supported targets.
		unsafe { raw::xchg(&key, &decay_ms) }
	}

	/// Returns the maximum retained-region growth increment in bytes.
	///
	/// Jemalloc exposes this control only when virtual memory retention is
	/// enabled.
	///
	/// # Errors
	///
	/// Returns `ENOENT` when retention is disabled, or another error if
	/// jemalloc rejects the arena or query.
	pub fn retain_grow_limit(&self) -> Result<usize> {
		let key = self.select(key::arena_retain_grow_limit()?);

		// SAFETY: `arena.<i>.retain_grow_limit` has C output type `size_t`.
		unsafe { raw::get(&key) }
	}

	/// Sets the maximum retained-region growth increment in bytes.
	///
	/// Jemalloc rounds accepted values down to a supported page-size class and
	/// returns the previous limit.
	///
	/// # Errors
	///
	/// Returns `ENOENT` when retention is disabled, or another error if
	/// jemalloc rejects the arena or limit.
	pub fn set_retain_grow_limit(&self, limit: usize) -> Result<usize> {
		let key = self.select(key::arena_retain_grow_limit()?);

		// SAFETY: `arena.<i>.retain_grow_limit` uses `size_t` for input and output.
		unsafe { raw::xchg(&key, &limit) }
	}

	/// Returns the raw extent hook table currently used for arena data.
	///
	/// The pointer remains owned by jemalloc or the application that installed
	/// it. Reading or retaining the pointee requires the corresponding lifetime
	/// and synchronization guarantees.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the arena, query, or a null result.
	pub fn extent_hooks(&self) -> Result<NonNull<RawExtentHooks>> {
		let key = self.select(key::arena_extent_hooks()?);

		// SAFETY: `arena.<i>.extent_hooks` returns an `extent_hooks_t *`.
		let hooks = unsafe { raw::get::<*mut ffi::extent_hooks_t>(&key) }?;

		NonNull::new(hooks).ok_or_else(Error::bad_address)
	}

	/// Replaces this arena's data extent hooks with a static Rust table.
	///
	/// The previous raw table pointer is returned without transferring
	/// ownership. The creation-time table can remain in use for arena
	/// metadata. Replacing hooks disables jemalloc's Huge Page Allocator for
	/// the arena; restoring the default table does not re-enable it.
	///
	/// # Errors
	///
	/// Returns an error if the table has no allocation callback or jemalloc
	/// rejects the arena or replacement.
	///
	/// # Safety
	///
	/// The replacement callbacks must manage every extant arena extent,
	/// commonly by forwarding unknown mappings to the previous table. They
	/// must satisfy [`Arena::create_with_extent_hooks`]'s callback contracts.
	/// Setting hooks on an uninitialized automatic arena initializes it, and
	/// jemalloc provides only best effort hook coverage for automatic arenas.
	pub unsafe fn set_extent_hooks<State: Sync + 'static>(
		&self,
		hooks: &'static ExtentHooks<State>,
	) -> Result<NonNull<RawExtentHooks>> {
		let hooks = hooks.as_raw();

		// SAFETY: the caller guarantees that the static replacement handles every
		// extant extent and satisfies all callback requirements.
		unsafe { self.set_raw_extent_hooks(hooks) }
	}

	/// Replaces this arena's data extent hooks from a raw table pointer.
	///
	/// The previous raw table pointer is returned without transferring
	/// ownership. This escape hatch also permits restoring jemalloc's default
	/// table. Replacing hooks disables jemalloc's Huge Page Allocator for the
	/// arena; restoring the default table does not re-enable it.
	///
	/// # Errors
	///
	/// Returns an error if the table lacks an allocation callback or jemalloc
	/// rejects the arena or replacement.
	///
	/// # Safety
	///
	/// The table must remain valid and immutable for the rest of the process.
	/// Its callbacks must manage every extant arena extent and satisfy the
	/// contracts of [`Arena::create_with_extent_hooks`]. Setting hooks on an
	/// uninitialized automatic arena initializes it, and automatic arenas offer
	/// only best effort hook coverage.
	pub unsafe fn set_raw_extent_hooks(
		&self,
		hooks: NonNull<RawExtentHooks>,
	) -> Result<NonNull<RawExtentHooks>> {
		// SAFETY: the caller guarantees that `hooks` points to a live table.
		if unsafe { hooks.as_ref() }.alloc.is_none() {
			return Err(Error::invalid_argument());
		}

		let key = self.select(key::arena_extent_hooks()?);
		let hooks = hooks.as_ptr();

		// SAFETY: `arena.<i>.extent_hooks` exchanges `extent_hooks_t *` values.
		// The new pointer satisfies its process-lifetime callback contract.
		let previous = unsafe { raw::xchg(&key, &hooks) }?;

		NonNull::new(previous).ok_or_else(Error::bad_address)
	}

	/// Destroys this explicitly created arena and consumes its owner.
	///
	/// Success invalidates the index and suppresses the fallback destructor. A
	/// failure returns both jemalloc's status and the unchanged handle for
	/// recovery. An owning handle remains owning after a failed command.
	///
	/// # Errors
	///
	/// Returns a recoverable error if this handle is non-owning or jemalloc
	/// cannot destroy the arena, including while any thread remains associated
	/// with it.
	///
	/// # Safety
	///
	/// No allocation from this arena may remain accessible if destruction
	/// succeeds. Every operation must be quiescent, and every automatic or
	/// explicit tcache that touched the arena must be flushed. Threads may
	/// remain associated when probing for the recoverable failure; jemalloc
	/// leaves the arena intact if it detects one.
	pub unsafe fn try_destroy(mut self) -> result::Result<(), ArenaDestroyError> {
		// SAFETY: the caller guarantees every destruction precondition.
		match unsafe { self.destroy() } {
			| Ok(()) => Ok(()),
			| Err(error) => Err(ArenaDestroyError::new(error, self)),
		}
	}

	/// Constructs one owner from a newly created C index.
	fn from_created_index(index: c_uint) -> Result<Self> {
		let index = usize::try_from(index).map_err(|_| Error::invalid_argument())?;
		validated_index(index)?;

		Ok(Self { index, owned: true })
	}

	/// Substitutes this arena's index into a fixed MIB template.
	fn select(&self, mut key: Key) -> Key {
		key[1] = self.index;
		key
	}

	/// Invokes a no-value command after selecting this arena.
	fn notify(&self, key: Key) -> Result {
		let key = self.select(key);

		// SAFETY: closed call sites provide complete arena command templates.
		// Replacing their numeric component preserves the no-value ABI.
		unsafe { raw::notify(&key) }
	}

	/// Destroys the owned arena and disarms this handle on success.
	unsafe fn destroy(&mut self) -> Result {
		if !self.owned {
			return Err(Error::invalid_argument());
		}

		self.notify(key::arena_destroy()?)?;
		self.owned = false;

		Ok(())
	}
}

impl Drop for Arena {
	fn drop(&mut self) {
		if self.owned {
			// SAFETY: safe code can create only an empty default-hook arena. Every
			// operation that can violate destruction requirements is unsafe and
			// carries the corresponding caller contract.
			let _: Result = unsafe { self.destroy() };
		}
	}
}

/// Converts a validated ordinary arena index to jemalloc's C representation.
pub(super) fn validated_index(index: usize) -> Result<c_uint> {
	if index >= ARENA_INDEX_LIMIT {
		return Err(Error::invalid_argument());
	}

	c_uint::try_from(index).map_err(|_| Error::invalid_argument())
}

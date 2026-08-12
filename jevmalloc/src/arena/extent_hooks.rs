//! Stable-address extent hook tables for explicit arenas.

use core::ptr::{NonNull, null_mut};

#[cfg(target_env = "msvc")]
use libc::c_int;
use libc::{c_uint, c_void, size_t};

use crate::ffi;

/// A raw C-compatible extent hook table.
///
/// Construct raw tables directly with a struct literal. Allocation is the only
/// mandatory callback when the table is installed in an arena.
pub type RawExtentHooks = ffi::extent_hooks_t;

/// An empty raw extent hook table suitable for constant struct updates.
///
/// This is the constant equivalent of [`RawExtentHooks::default`]. Set at least
/// the allocation callback before installing the resulting table.
pub const EMPTY_RAW_EXTENT_HOOKS: RawExtentHooks = RawExtentHooks {
	alloc: None,
	dalloc: None,
	destroy: None,
	commit: None,
	decommit: None,
	purge_lazy: None,
	purge_forced: None,
	split: None,
	merge: None,
};

/// An allocation request from jemalloc.
///
/// `zeroed` and `committed` state requirements must remain satisfied by a
/// successful [`ExtentAllocation`].
#[derive(Clone, Copy, Debug)]
pub struct ExtentAlloc {
	/// Required address, or no fixed address when jemalloc has no preference.
	pub address: Option<NonNull<u8>>,

	/// Requested mapping size in bytes.
	pub size: usize,

	/// Required base-address alignment in bytes.
	pub alignment: usize,

	/// Whether jemalloc requires zero-filled memory.
	pub zeroed: bool,

	/// Whether jemalloc requires committed memory.
	pub committed: bool,

	/// Arena requesting the mapping.
	pub arena: c_uint,
}

/// A mapping returned by an allocation callback.
///
/// The address must satisfy the request's size, alignment, and optional fixed
/// address. The state flags describe the returned mapping.
#[derive(Clone, Copy, Debug)]
pub struct ExtentAllocation {
	/// Base address of the new mapping.
	pub address: NonNull<u8>,

	/// Whether the mapping is zero-filled.
	pub zeroed: bool,

	/// Whether the mapping is committed.
	pub committed: bool,
}

/// An extent passed to deallocation or destruction.
///
/// The callback receives the mapping's address, size, commit state, and owning
/// arena without creating a Rust reference to its storage.
#[derive(Clone, Copy, Debug)]
pub struct Extent {
	/// Base address of the mapping.
	pub address: NonNull<u8>,

	/// Mapping size in bytes.
	pub size: usize,

	/// Whether the mapping is committed.
	pub committed: bool,

	/// Arena owning the mapping.
	pub arena: c_uint,
}

/// A subrange passed to a commit, decommit, or purge callback.
///
/// The requested range starts at `offset` within the complete mapping and
/// extends for `length` bytes.
#[derive(Clone, Copy, Debug)]
pub struct ExtentRange {
	/// Base address of the complete mapping.
	pub address: NonNull<u8>,

	/// Complete mapping size in bytes.
	pub size: usize,

	/// Byte offset of the requested range.
	pub offset: usize,

	/// Requested range length in bytes.
	pub length: usize,

	/// Arena owning the mapping.
	pub arena: c_uint,
}

/// An extent split request.
///
/// The two requested sizes are adjacent partitions of the complete mapping.
#[derive(Clone, Copy, Debug)]
pub struct ExtentSplit {
	/// Base address of the complete mapping.
	pub address: NonNull<u8>,

	/// Complete mapping size in bytes.
	pub size: usize,

	/// Size of the first resulting extent.
	pub first_size: usize,

	/// Size of the second resulting extent.
	pub second_size: usize,

	/// Whether the mapping is committed.
	pub committed: bool,

	/// Arena owning the mapping.
	pub arena: c_uint,
}

/// An adjacent extent merge request.
///
/// Both mappings belong to the same arena and have the same commit state.
#[derive(Clone, Copy, Debug)]
pub struct ExtentMerge {
	/// Base address of the first mapping.
	pub first_address: NonNull<u8>,

	/// Size of the first mapping in bytes.
	pub first_size: usize,

	/// Base address of the second mapping.
	pub second_address: NonNull<u8>,

	/// Size of the second mapping in bytes.
	pub second_size: usize,

	/// Whether both mappings are committed.
	pub committed: bool,

	/// Arena owning both mappings.
	pub arena: c_uint,
}

/// The outcome of an extent operation with an error return.
///
/// Jemalloc represents success as `false` and failure or opt-out as `true`.
/// This type keeps that inverted convention outside Rust callbacks.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtentHookResult {
	/// The requested operation succeeded.
	Success,

	/// The operation failed or the callback declined it.
	Failure,
}

/// Rust allocation callback with access to table-local state.
///
/// The callback may run concurrently and must satisfy the raw allocation-hook
/// contract without unwinding.
pub type ExtentAllocFn<State> = unsafe fn(&State, ExtentAlloc) -> Option<ExtentAllocation>;

/// Rust deallocation callback with access to table-local state.
///
/// Returning [`ExtentHookResult::Failure`] retains the mapping for later reuse.
pub type ExtentDallocFn<State> = unsafe fn(&State, Extent) -> ExtentHookResult;

/// Rust destruction callback with access to table-local state.
///
/// Destruction is unconditional when jemalloc invokes the callback.
pub type ExtentDestroyFn<State> = unsafe fn(&State, Extent);

/// Rust range callback with access to table-local state.
///
/// Commit, decommit, and both purge operations share this request shape.
pub type ExtentRangeFn<State> = unsafe fn(&State, ExtentRange) -> ExtentHookResult;

/// Rust split callback with access to table-local state.
///
/// Returning [`ExtentHookResult::Failure`] leaves the mapping whole.
pub type ExtentSplitFn<State> = unsafe fn(&State, ExtentSplit) -> ExtentHookResult;

/// Rust merge callback with access to table-local state.
///
/// Returning [`ExtentHookResult::Failure`] leaves both mappings separate.
pub type ExtentMergeFn<State> = unsafe fn(&State, ExtentMerge) -> ExtentHookResult;

/// Rust callbacks used by a stateful extent hook table.
///
/// Construct this value with a struct literal. [`ExtentCallbacks::EMPTY`] is
/// available for constants, while [`Default::default`] serves runtime values.
#[derive(Debug)]
pub struct ExtentCallbacks<State> {
	/// Mapping allocation callback.
	pub alloc: Option<ExtentAllocFn<State>>,

	/// Mapping deallocation callback.
	pub dalloc: Option<ExtentDallocFn<State>>,

	/// Unconditional mapping destruction callback.
	pub destroy: Option<ExtentDestroyFn<State>>,

	/// Physical memory commit callback.
	pub commit: Option<ExtentRangeFn<State>>,

	/// Physical memory decommit callback.
	pub decommit: Option<ExtentRangeFn<State>>,

	/// Lazy page purge callback.
	pub purge_lazy: Option<ExtentRangeFn<State>>,

	/// Forced page purge callback.
	pub purge_forced: Option<ExtentRangeFn<State>>,

	/// Extent split callback.
	pub split: Option<ExtentSplitFn<State>>,

	/// Adjacent extent merge callback.
	pub merge: Option<ExtentMergeFn<State>>,
}

impl<State> Clone for ExtentCallbacks<State> {
	fn clone(&self) -> Self { *self }
}

impl<State> Copy for ExtentCallbacks<State> {}

impl<State> ExtentCallbacks<State> {
	/// An extent callback set with every operation disabled.
	///
	/// Use this value for constant struct updates, then set at least `alloc`
	/// before installing the table.
	pub const EMPTY: Self = Self {
		alloc: None,
		dalloc: None,
		destroy: None,
		commit: None,
		decommit: None,
		purge_lazy: None,
		purge_forced: None,
		split: None,
		merge: None,
	};
}

impl<State> Default for ExtentCallbacks<State> {
	fn default() -> Self { Self::EMPTY }
}

/// A stable-address extent hook table with Rust-owned callback state.
///
/// Build the table in static storage before installing it in an arena. Jemalloc
/// may invoke its callbacks concurrently until successful arena destruction.
#[repr(C)]
#[derive(Debug)]
pub struct ExtentHooks<State> {
	/// C-compatible header passed to jemalloc and back to each trampoline.
	raw: RawExtentHooks,

	/// Rust callback targets selected before installation.
	callbacks: ExtentCallbacks<State>,

	/// Immutable callback context with internally synchronized mutable state.
	state: State,
}

impl<State: 'static> ExtentHooks<State> {
	/// Constructs a stateful hook table from a literal callback set.
	///
	/// The callback set becomes immutable with the table. Arena installation
	/// rejects a table whose allocation callback is absent.
	#[must_use]
	pub const fn new(state: State, callbacks: ExtentCallbacks<State>) -> Self {
		let raw = RawExtentHooks {
			alloc: match callbacks.alloc {
				| Some(_) => Some(alloc::<State>),
				| None => None,
			},
			dalloc: match callbacks.dalloc {
				| Some(_) => Some(dalloc::<State>),
				| None => None,
			},
			destroy: match callbacks.destroy {
				| Some(_) => Some(destroy::<State>),
				| None => None,
			},
			commit: match callbacks.commit {
				| Some(_) => Some(commit::<State>),
				| None => None,
			},
			decommit: match callbacks.decommit {
				| Some(_) => Some(decommit::<State>),
				| None => None,
			},
			purge_lazy: match callbacks.purge_lazy {
				| Some(_) => Some(purge_lazy::<State>),
				| None => None,
			},
			purge_forced: match callbacks.purge_forced {
				| Some(_) => Some(purge_forced::<State>),
				| None => None,
			},
			split: match callbacks.split {
				| Some(_) => Some(split::<State>),
				| None => None,
			},
			merge: match callbacks.merge {
				| Some(_) => Some(merge::<State>),
				| None => None,
			},
		};

		Self { raw, callbacks, state }
	}

	/// Returns the immutable callback state.
	///
	/// State shared across callback threads must provide its own interior
	/// synchronization.
	#[must_use]
	#[inline]
	pub const fn state(&self) -> &State { &self.state }

	/// Returns the configured Rust callback set.
	///
	/// The returned callbacks match the C slots selected during construction.
	#[must_use]
	#[inline]
	pub const fn callbacks(&self) -> &ExtentCallbacks<State> { &self.callbacks }

	/// Returns the generated raw hook table.
	///
	/// Copying this header away from its enclosing [`ExtentHooks`] value makes
	/// its trampolines invalid and must never be used for arena installation.
	#[must_use]
	#[inline]
	pub const fn raw(&self) -> &RawExtentHooks { &self.raw }
}

impl<State: Sync + 'static> ExtentHooks<State> {
	/// Returns the stable pointer installed in an arena.
	///
	/// The pointer retains provenance for the complete enclosing table. The
	/// `raw` header is its first field under `repr(C)`.
	#[must_use]
	#[inline]
	pub fn as_raw(&'static self) -> NonNull<RawExtentHooks> { NonNull::from(self).cast() }
}

/// Returns the Rust table enclosing a raw header passed to a trampoline.
///
/// # Safety
///
/// `raw` must point to the first field of a live, immutable
/// `ExtentHooks<State>` value with static storage duration.
#[inline]
unsafe fn from_raw<State: 'static>(raw: *mut RawExtentHooks) -> &'static ExtentHooks<State> {
	// SAFETY: the caller guarantees the pointer came from this table's leading
	// `raw` field and that the complete table remains live.
	unsafe { &*raw.cast::<ExtentHooks<State>>() }
}

/// Converts a non-null raw mapping address into its Rust representation.
///
/// # Safety
///
/// `address` must be non-null as required by the jemalloc callback contract.
#[inline]
unsafe fn address(address: *mut c_void) -> NonNull<u8> {
	// SAFETY: the caller guarantees a non-null mapping address.
	unsafe { NonNull::new_unchecked(address.cast::<u8>()) }
}

/// Invokes the Rust allocation callback and translates its result to the ABI.
unsafe extern "C" fn alloc<State: 'static>(
	raw: *mut RawExtentHooks,
	new_address: *mut c_void,
	size: size_t,
	alignment: size_t,
	zeroed: *mut CBool,
	committed: *mut CBool,
	arena: c_uint,
) -> *mut c_void {
	// SAFETY: jemalloc passes back the installed leading raw header.
	let hooks = unsafe { from_raw::<State>(raw) };
	let Some(callback) = hooks.callbacks.alloc else {
		return null_mut();
	};
	let request = ExtentAlloc {
		address: NonNull::new(new_address.cast::<u8>()),
		size,
		alignment,
		// SAFETY: jemalloc supplies live boolean input and output pointers.
		zeroed: unsafe { read_bool(zeroed) },
		// SAFETY: jemalloc supplies live boolean input and output pointers.
		committed: unsafe { read_bool(committed) },
		arena,
	};

	// SAFETY: jemalloc supplied a valid request and the callback owns the
	// mapping contract documented by `ExtentAllocFn`.
	let Some(allocation) = (unsafe { callback(&hooks.state, request) }) else {
		return null_mut();
	};

	debug_assert!(
		request
			.address
			.is_none_or(|address| address == allocation.address)
	);
	debug_assert!(!request.zeroed || allocation.zeroed);
	debug_assert!(!request.committed || allocation.committed);

	// SAFETY: jemalloc supplied a live boolean output pointer.
	unsafe { write_bool(zeroed, allocation.zeroed) };
	// SAFETY: jemalloc supplied a live boolean output pointer.
	unsafe { write_bool(committed, allocation.committed) };

	allocation.address.as_ptr().cast::<c_void>()
}

/// Invokes the Rust deallocation callback and translates its result to the ABI.
unsafe extern "C" fn dalloc<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	committed: CBool,
	arena: c_uint,
) -> CBool {
	// SAFETY: jemalloc passes back the installed leading raw header.
	let hooks = unsafe { from_raw::<State>(raw) };
	let Some(callback) = hooks.callbacks.dalloc else {
		return failure();
	};
	let extent = Extent {
		// SAFETY: jemalloc supplies a non-null mapping address.
		address: unsafe { self::address(address) },
		size,
		committed: from_bool(committed),
		arena,
	};

	// SAFETY: jemalloc supplied a valid extent and the callback owns its
	// deallocation contract.
	let result = unsafe { callback(&hooks.state, extent) };

	to_bool(result == ExtentHookResult::Failure)
}

/// Invokes the Rust unconditional destruction callback.
unsafe extern "C" fn destroy<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	committed: CBool,
	arena: c_uint,
) {
	// SAFETY: jemalloc passes back the installed leading raw header.
	let hooks = unsafe { from_raw::<State>(raw) };
	let Some(callback) = hooks.callbacks.destroy else {
		return;
	};
	let extent = Extent {
		// SAFETY: jemalloc supplies a non-null mapping address.
		address: unsafe { self::address(address) },
		size,
		committed: from_bool(committed),
		arena,
	};

	// SAFETY: jemalloc supplied a valid extent and the callback owns its
	// unconditional destruction contract.
	unsafe { callback(&hooks.state, extent) };
}

/// Invokes the Rust commit callback and translates its result to the ABI.
unsafe extern "C" fn commit<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena: c_uint,
) -> CBool {
	// SAFETY: the arguments originate from jemalloc's commit path.
	unsafe { range::<State>(raw, address, size, offset, length, arena, RangeKind::Commit) }
}

/// Invokes the Rust decommit callback and translates its result to the ABI.
unsafe extern "C" fn decommit<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena: c_uint,
) -> CBool {
	// SAFETY: the arguments originate from jemalloc's decommit path.
	unsafe { range::<State>(raw, address, size, offset, length, arena, RangeKind::Decommit) }
}

/// Invokes the Rust lazy purge callback and translates its result to the ABI.
unsafe extern "C" fn purge_lazy<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena: c_uint,
) -> CBool {
	// SAFETY: the arguments originate from jemalloc's lazy purge path.
	unsafe { range::<State>(raw, address, size, offset, length, arena, RangeKind::PurgeLazy) }
}

/// Invokes the Rust forced purge callback and translates its result to the ABI.
unsafe extern "C" fn purge_forced<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena: c_uint,
) -> CBool {
	// SAFETY: the arguments originate from jemalloc's forced purge path.
	unsafe { range::<State>(raw, address, size, offset, length, arena, RangeKind::PurgeForced) }
}

/// Selects one of the four callbacks sharing the extent-range ABI.
#[derive(Clone, Copy, Debug)]
enum RangeKind {
	/// Physical memory commit.
	Commit,

	/// Physical memory decommit.
	Decommit,

	/// Lazy page purge.
	PurgeLazy,

	/// Forced page purge.
	PurgeForced,
}

/// Invokes one of the Rust callbacks sharing the extent-range ABI.
#[inline]
unsafe fn range<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena: c_uint,
	kind: RangeKind,
) -> CBool {
	// SAFETY: jemalloc passes back the installed leading raw header.
	let hooks = unsafe { from_raw::<State>(raw) };
	let callback = match kind {
		| RangeKind::Commit => hooks.callbacks.commit,
		| RangeKind::Decommit => hooks.callbacks.decommit,
		| RangeKind::PurgeLazy => hooks.callbacks.purge_lazy,
		| RangeKind::PurgeForced => hooks.callbacks.purge_forced,
	};
	let Some(callback) = callback else {
		return failure();
	};
	let range = ExtentRange {
		// SAFETY: jemalloc supplies a non-null mapping address.
		address: unsafe { self::address(address) },
		size,
		offset,
		length,
		arena,
	};

	// SAFETY: jemalloc supplied a valid range and the callback owns the selected
	// operation's contract.
	let result = unsafe { callback(&hooks.state, range) };

	to_bool(result == ExtentHookResult::Failure)
}

/// Invokes the Rust split callback and translates its result to the ABI.
unsafe extern "C" fn split<State: 'static>(
	raw: *mut RawExtentHooks,
	address: *mut c_void,
	size: size_t,
	first_size: size_t,
	second_size: size_t,
	committed: CBool,
	arena: c_uint,
) -> CBool {
	// SAFETY: jemalloc passes back the installed leading raw header.
	let hooks = unsafe { from_raw::<State>(raw) };
	let Some(callback) = hooks.callbacks.split else {
		return failure();
	};
	let split = ExtentSplit {
		// SAFETY: jemalloc supplies a non-null mapping address.
		address: unsafe { self::address(address) },
		size,
		first_size,
		second_size,
		committed: from_bool(committed),
		arena,
	};

	// SAFETY: jemalloc supplied a valid split and the callback owns the mapping
	// partition contract.
	let result = unsafe { callback(&hooks.state, split) };

	to_bool(result == ExtentHookResult::Failure)
}

/// Invokes the Rust merge callback and translates its result to the ABI.
unsafe extern "C" fn merge<State: 'static>(
	raw: *mut RawExtentHooks,
	first_address: *mut c_void,
	first_size: size_t,
	second_address: *mut c_void,
	second_size: size_t,
	committed: CBool,
	arena: c_uint,
) -> CBool {
	// SAFETY: jemalloc passes back the installed leading raw header.
	let hooks = unsafe { from_raw::<State>(raw) };
	let Some(callback) = hooks.callbacks.merge else {
		return failure();
	};
	let merge = ExtentMerge {
		// SAFETY: jemalloc supplies a non-null first mapping address.
		first_address: unsafe { address(first_address) },
		first_size,
		// SAFETY: jemalloc supplies a non-null second mapping address.
		second_address: unsafe { address(second_address) },
		second_size,
		committed: from_bool(committed),
		arena,
	};

	// SAFETY: jemalloc supplied valid adjacent extents and the callback owns the
	// merge contract.
	let result = unsafe { callback(&hooks.state, merge) };

	to_bool(result == ExtentHookResult::Failure)
}

/// Jemalloc's C boolean representation under cl.exe.
#[cfg(target_env = "msvc")]
type CBool = c_int;

/// Jemalloc's C boolean representation on non-MSVC targets.
#[cfg(not(target_env = "msvc"))]
type CBool = bool;

/// Converts a C callback boolean to Rust.
#[cfg(target_env = "msvc")]
const fn from_bool(value: CBool) -> bool { value != 0 }

/// Converts a C callback boolean to Rust.
#[cfg(not(target_env = "msvc"))]
const fn from_bool(value: CBool) -> bool { value }

/// Converts a Rust callback boolean to C.
#[cfg(target_env = "msvc")]
const fn to_bool(value: bool) -> CBool {
	match value {
		| true => 1,
		| false => 0,
	}
}

/// Converts a Rust callback boolean to C.
#[cfg(not(target_env = "msvc"))]
const fn to_bool(value: bool) -> CBool { value }

/// Reads a C callback boolean through a live pointer.
///
/// # Safety
///
/// `value` must be valid and aligned for one `CBool` read.
unsafe fn read_bool(value: *const CBool) -> bool {
	// SAFETY: the caller guarantees a live and aligned input pointer.
	from_bool(unsafe { value.read() })
}

/// Writes a C callback boolean through a live pointer.
///
/// # Safety
///
/// `value` must be valid and aligned for one `CBool` write.
unsafe fn write_bool(value: *mut CBool, state: bool) {
	// SAFETY: the caller guarantees a live and aligned output pointer.
	unsafe { value.write(to_bool(state)) };
}

/// Returns the C callback representation of failure or opt-out.
const fn failure() -> CBool { to_bool(true) }

#[cfg(test)]
mod tests;

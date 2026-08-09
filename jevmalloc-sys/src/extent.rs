//! Extent hooks, the arena-level virtual memory interface.
//!
//! An arena reaches every mapping it owns through this table, from allocation
//! through commit and purge to destruction. Everything but allocation can be
//! opted out of, and the table must outlive every arena holding it.

#[cfg(target_env = "msvc")]
use libc::c_int;
use libc::{c_uint, c_void, size_t};

// jemalloc's own `stdbool.h`, reached only under MSVC, defines `bool` as
// `BOOL`, which is `int`; everywhere else Rust's `bool` matches.
#[cfg(target_env = "msvc")]
type c_bool = c_int;
#[cfg(not(target_env = "msvc"))]
type c_bool = bool;

/// Extent lifetime management functions.
pub type extent_hooks_t = extent_hooks_s;

// Two variants of the struct: the `jevmalloc_docs` one names the type aliases
// so rustdoc renders them, and the normal one spells the `fn` types out.

#[repr(C)]
#[cfg(not(jevmalloc_docs))]
#[doc(hidden)]
#[allow(missing_docs)]
#[derive(Copy, Clone, Default)]
pub struct extent_hooks_s {
	pub alloc: Option<
		unsafe extern "C" fn(
			*mut Self,
			*mut c_void,
			size_t,
			size_t,
			*mut c_bool,
			*mut c_bool,
			c_uint,
		) -> *mut c_void,
	>,
	pub dalloc:
		Option<unsafe extern "C" fn(*mut Self, *mut c_void, size_t, c_bool, c_uint) -> c_bool>,
	pub destroy: Option<unsafe extern "C" fn(*mut Self, *mut c_void, size_t, c_bool, c_uint)>,
	pub commit: Option<
		unsafe extern "C" fn(*mut Self, *mut c_void, size_t, size_t, size_t, c_uint) -> c_bool,
	>,
	pub decommit: Option<
		unsafe extern "C" fn(*mut Self, *mut c_void, size_t, size_t, size_t, c_uint) -> c_bool,
	>,
	pub purge_lazy: Option<
		unsafe extern "C" fn(*mut Self, *mut c_void, size_t, size_t, size_t, c_uint) -> c_bool,
	>,
	pub purge_forced: Option<
		unsafe extern "C" fn(*mut Self, *mut c_void, size_t, size_t, size_t, c_uint) -> c_bool,
	>,
	pub split: Option<
		unsafe extern "C" fn(
			*mut Self,
			*mut c_void,
			size_t,
			size_t,
			size_t,
			c_bool,
			c_uint,
		) -> c_bool,
	>,
	pub merge: Option<
		unsafe extern "C" fn(
			*mut Self,
			*mut c_void,
			size_t,
			*mut c_void,
			size_t,
			c_bool,
			c_uint,
		) -> c_bool,
	>,
}

/// Extent lifetime management functions.
///
/// The extent_hooks_t structure comprises function pointers which are described
/// individually below. `jemalloc` uses these functions to manage extent
/// lifetime, which starts off with allocation of mapped committed memory, in
/// the simplest case followed by deallocation. However, there are performance
/// and platform reasons to retain extents for later reuse. Cleanup attempts
/// cascade from deallocation to decommit to forced purging to lazy purging,
/// which gives the extent management functions opportunities to reject the most
/// permanent cleanup operations in favor of less permanent (and often less
/// costly) operations. All operations except allocation can be universally
/// opted out of by setting the hook pointers to `NULL`, or selectively opted
/// out of by returning failure. Note that once the extent hook is set, the
/// structure is accessed directly by the associated arenas, so it must remain
/// valid for the entire lifetime of the arenas.
#[repr(C)]
#[cfg(jevmalloc_docs)]
#[derive(Copy, Clone, Default)]
pub struct extent_hooks_s {
	#[allow(missing_docs)]
	pub alloc: Option<extent_alloc_t>,
	#[allow(missing_docs)]
	pub dalloc: Option<extent_dalloc_t>,
	#[allow(missing_docs)]
	pub destroy: Option<extent_destroy_t>,
	#[allow(missing_docs)]
	pub commit: Option<extent_commit_t>,
	#[allow(missing_docs)]
	pub decommit: Option<extent_decommit_t>,
	#[allow(missing_docs)]
	pub purge_lazy: Option<extent_purge_t>,
	#[allow(missing_docs)]
	pub purge_forced: Option<extent_purge_t>,
	#[allow(missing_docs)]
	pub split: Option<extent_split_t>,
	#[allow(missing_docs)]
	pub merge: Option<extent_merge_t>,
}

/// Extent allocation function.
///
/// On success returns a pointer to `size` bytes of mapped memory on behalf of
/// arena `arena_ind` such that the extent's base address is a multiple of
/// `alignment`, as well as setting `*zero` to indicate whether the extent is
/// zeroed and `*commit` to indicate whether the extent is committed.
///
/// Zeroing is mandatory if `*zero` is `true` upon function entry. Committing is
/// mandatory if `*commit` is true upon function entry. If `new_addr` is not
/// null, the returned pointer must be `new_addr` on success or null on error.
///
/// Committed memory may be committed in absolute terms as on a system that does
/// not overcommit, or in implicit terms as on a system that overcommits and
/// satisfies physical memory needs on demand via soft page faults. Note that
/// replacing the default extent allocation function makes the arena's
/// `arena.<i>.dss` setting irrelevant.
///
/// # Errors
///
/// On error the function returns null and leaves `*zero` and `*commit`
/// unmodified.
///
/// # Safety
///
/// The behavior is _undefined_ if:
///
/// * the `size` parameter is not a multiple of the page size
/// * the `alignment` parameter is not a power of two at least as large as the
///   page size
pub type extent_alloc_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	new_addr: *mut c_void,
	size: size_t,
	alignment: size_t,
	zero: *mut c_bool,
	commit: *mut c_bool,
	arena_ind: c_uint,
) -> *mut c_void;

/// Extent deallocation function.
///
/// Deallocates an extent at given `addr` and `size` with `committed`/decommited
/// memory as indicated, on behalf of arena `arena_ind`, returning `false` upon
/// success.
///
/// If the function returns `true`, this indicates opt-out from deallocation;
/// the virtual memory mapping associated with the extent remains mapped, in the
/// same commit state, and available for future use, in which case it will be
/// automatically retained for later reuse.
pub type extent_dalloc_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	committed: c_bool,
	arena_ind: c_uint,
) -> c_bool;

/// Extent destruction function.
///
/// Unconditionally destroys an extent at given `addr` and `size` with
/// `committed`/decommited memory as indicated, on behalf of arena `arena_ind`.
///
/// This function may be called to destroy retained extents during arena
/// destruction (see `arena.<i>.destroy`).
pub type extent_destroy_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	committed: c_bool,
	arena_ind: c_uint,
);

/// Extent commit function.
///
/// Commits zeroed physical memory to back pages within an extent at given
/// `addr` and `size` at `offset` bytes, extending for `length` on behalf of
/// arena `arena_ind`, returning `false` upon success.
///
/// Committed memory may be committed in absolute terms as on a system that does
/// not overcommit, or in implicit terms as on a system that overcommits and
/// satisfies physical memory needs on demand via soft page faults. If the
/// function returns `true`, this indicates insufficient physical memory to
/// satisfy the request.
pub type extent_commit_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena_ind: c_uint,
) -> c_bool;

/// Extent decommit function.
///
/// Decommits any physical memory that is backing pages within an extent at
/// given `addr` and `size` at `offset` bytes, extending for `length` on behalf
/// of arena `arena_ind`, returning `false` upon success, in which case the
/// pages will be committed via the extent commit function before being reused.
///
/// If the function returns `true`, this indicates opt-out from decommit; the
/// memory remains committed and available for future use, in which case it will
/// be automatically retained for later reuse.
pub type extent_decommit_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena_ind: c_uint,
) -> c_bool;

/// Extent purge function.
///
/// Discards physical pages within the virtual memory mapping associated with an
/// extent at given `addr` and `size` at `offset` bytes, extending for `length`
/// on behalf of arena `arena_ind`.
///
/// A lazy extent purge function (e.g. implemented via `madvise(...MADV_FREE)`)
/// can delay purging indefinitely and leave the pages within the purged virtual
/// memory range in an indeterminite state, whereas a forced extent purge
/// function immediately purges, and the pages within the virtual memory range
/// will be zero-filled the next time they are accessed. If the function returns
/// `true`, this indicates failure to purge.
pub type extent_purge_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	offset: size_t,
	length: size_t,
	arena_ind: c_uint,
) -> c_bool;

/// Extent split function.
///
/// Optionally splits an extent at given `addr` and `size` into two adjacent
/// extents, the first of `size_a` bytes, and the second of `size_b` bytes,
/// operating on `committed`/decommitted memory as indicated, on behalf of arena
/// `arena_ind`, returning `false` upon success.
///
/// If the function returns `true`, this indicates that the extent remains
/// unsplit and therefore should continue to be operated on as a whole.
pub type extent_split_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	size_a: size_t,
	size_b: size_t,
	committed: c_bool,
	arena_ind: c_uint,
) -> c_bool;

/// Extent merge function.
///
/// Optionally merges adjacent extents, at given `addr_a` and `size_a` with
/// given `addr_b` and `size_b` into one contiguous extent, operating on
/// `committed`/decommitted memory as indicated, on behalf of arena `arena_ind`,
/// returning `false` upon success.
///
/// If the function returns `true`, this indicates that the extents remain
/// distinct mappings and therefore should continue to be operated on
/// independently.
pub type extent_merge_t = unsafe extern "C" fn(
	extent_hooks: *mut extent_hooks_t,
	addr_a: *mut c_void,
	size_a: size_t,
	addr_b: *mut c_void,
	size_b: size_t,
	committed: c_bool,
	arena_ind: c_uint,
) -> c_bool;

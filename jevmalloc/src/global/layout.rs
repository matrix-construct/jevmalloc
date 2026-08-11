// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Layout normalization and flag selection for jemalloc calls.
//!
//! Requests smaller than the platform's fundamental allocation quantum are
//! raised to it before reaching jemalloc. Flag selection omits redundant
//! alignment bits so ordinary sized deallocations remain eligible for
//! jemalloc's thread-cache fast path.

use libc::{c_int, c_void};

use super::{Layout, assert_unchecked, cmp, ffi, ffi::MALLOCX_ALIGN};

/// Gives the platform's fundamental allocation alignment.
///
/// This matches C's `_Alignof(max_align_t)` and is the minimum alignment that
/// the standard allocation functions guarantee for a sufficiently large
/// request. Layout normalization raises smaller request sizes to this value.
pub const QUANTUM: usize = QUANTUM_VALUE;

/// Uses an eight-byte quantum on supported 32-bit targets.
///
/// These targets define `_Alignof(max_align_t)` as eight bytes for the
/// configurations supported by this crate.
#[cfg(any(
	target_arch = "arm",
	target_arch = "mips",
	target_arch = "powerpc"
))]
const QUANTUM_VALUE: usize = 8;

/// Uses a sixteen-byte quantum on the listed supported targets.
///
/// These targets define `_Alignof(max_align_t)` as sixteen bytes for the
/// configurations supported by this crate.
#[cfg(any(
	target_arch = "x86",
	target_arch = "x86_64",
	target_arch = "aarch64",
	target_arch = "powerpc64",
	target_arch = "loongarch64",
	target_arch = "mips64",
	target_arch = "riscv64",
	target_arch = "s390x",
	target_arch = "sparc64"
))]
const QUANTUM_VALUE: usize = 16;

/// Raises a nonzero layout's size to the platform's allocation quantum.
///
/// The size becomes at least [`QUANTUM`], while the requested alignment is
/// preserved. The result is not rounded to a jemalloc size class. Preserving
/// the alignment keeps every valid nonzero input representable as a [`Layout`].
///
/// # Safety
///
/// The input size must be nonzero.
#[inline]
#[must_use]
pub unsafe fn adjust_layout(layout: Layout) -> Layout {
	// SAFETY: the caller guarantees a nonzero input size.
	unsafe { assert_unchecked(layout.size() > 0) };
	let size = cmp::max(layout.size(), QUANTUM);

	// SAFETY: increasing a nonzero size below `QUANTUM` preserves validity.
	// Alignments at most `QUANTUM` divide the new size; larger alignments round
	// both the original and adjusted sizes to the same value.
	unsafe { Layout::from_size_align_unchecked(size, layout.align()) }
}

/// Computes the jemalloc flag word required by a layout.
///
/// A zero word lets sized deallocation use jemalloc's thread-cache fast path
/// when the size class already implies the requested alignment. An explicit
/// `MALLOCX_ALIGN` value is retained for over-aligned layouts and for raw
/// layouts whose alignment exceeds their size.
#[inline]
#[must_use]
pub fn layout_flags(layout: Layout) -> c_int {
	let implied = layout.align() <= QUANTUM && layout.align() <= layout.size();

	if implied { 0 } else { MALLOCX_ALIGN(layout.align()) }
}

/// Returns the usable size associated with an allocation pointer.
///
/// The reported size can exceed the original request and is intended only for
/// introspection, not as a stable size-class guarantee. A null pointer returns
/// zero without querying jemalloc. Non-null pointers are queried through
/// `sallocx` so the allocator and ownership lookup cannot be split by symbol
/// interposition.
///
/// # Safety
///
/// A non-null `ptr` must identify a live allocation owned by the jemalloc
/// instance linked into this process.
#[inline]
pub unsafe fn usable_size<T>(ptr: *const T) -> usize {
	if ptr.is_null() {
		0
	} else {
		// SAFETY: the function contract guarantees a live non-null allocation
		// owned by the linked jemalloc instance.
		unsafe { ffi::sallocx(ptr.cast::<c_void>(), 0) }
	}
}

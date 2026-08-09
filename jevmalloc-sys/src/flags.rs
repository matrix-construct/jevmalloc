//! Flag bits for the `flags` argument of the extended allocation functions.
//!
//! Combine them with a bitwise or. `jemalloc` decodes the word only when it is
//! non-zero, so an empty one, meaning default behavior throughout, skips the
//! alignment, zeroing, thread-cache and arena lookups altogether.

use libc::c_int;

/// Align the memory allocation to start at an address that is a
/// multiple of `1 << la`.
///
/// # Safety
///
/// It does not validate that `la` is within the valid range.
#[inline]
#[must_use]
pub const fn MALLOCX_LG_ALIGN(la: usize) -> c_int { la as c_int }

/// Align the memory allocation to start at an address that is a multiple of
/// `align`, where `align` is a power of two.
///
/// # Safety
///
/// This function does not validate that `align` is a power of two.
#[inline]
#[must_use]
pub const fn MALLOCX_ALIGN(align: usize) -> c_int { align.trailing_zeros() as c_int }

/// Initialize newly allocated memory to contain zero bytes.
///
/// In the growing reallocation case, the real size prior to reallocation
/// defines the boundary between untouched bytes and those that are initialized
/// to contain zero bytes.
///
/// If this option is not set, newly allocated memory is uninitialized.
pub const MALLOCX_ZERO: c_int = 0x40;

/// Use the thread-specific cache (_tcache_) specified by the identifier `tc`.
///
/// # Safety
///
/// `tc` must have been acquired via the `tcache.create mallctl`. This function
/// does not validate that `tc` specifies a valid identifier.
#[inline]
#[must_use]
pub const fn MALLOCX_TCACHE(tc: usize) -> c_int { tc.wrapping_add(2).wrapping_shl(8) as c_int }

/// Do not use a thread-specific cache (_tcache_).
///
/// Unless [`MALLOCX_TCACHE`] or [`MALLOCX_TCACHE_NONE`] is specified, an
/// automatically managed _tcache_ will be used under many circumstances.
///
/// # Safety
///
/// This option cannot be used in the same `flags` argument as
/// [`MALLOCX_TCACHE`].
pub const MALLOCX_TCACHE_NONE: c_int = MALLOCX_TCACHE(usize::MAX);

/// Use the arena specified by the index `a`.
///
/// This option has no effect for regions that were allocated via an arena other
/// than the one specified.
///
/// # Safety
///
/// This function does not validate that `a` specifies an arena index in the
/// valid range.
#[inline]
#[must_use]
pub const fn MALLOCX_ARENA(a: usize) -> c_int { (a as c_int).wrapping_add(1).wrapping_shl(20) }

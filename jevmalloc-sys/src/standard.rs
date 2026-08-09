//! The ISO C and POSIX allocation entry points.
//!
//! These are the names a C program reaches without knowing `jemalloc` is
//! underneath, so on a supported platform the default feature set links them
//! unprefixed and lets them service libc's own allocations too. Elsewhere, and
//! whenever that feature is off, every symbol takes the `_rjem_` prefix and
//! libc keeps its own heap.

use libc::{c_int, c_void, size_t};

unsafe extern "C" {
	/// Allocates `size` bytes of uninitialized memory.
	///
	/// It returns a pointer to the start (lowest byte address) of the allocated
	/// space. This pointer is suitably aligned so that it may be assigned to a
	/// pointer to any type of object and then used to access such an object in
	/// the space allocated until the space is explicitly deallocated. Each
	/// yielded pointer points to an object disjoint from any other object.
	///
	/// If the `size` of the space requested is zero, either a null pointer is
	/// returned, or the behavior is as if the `size` were some nonzero value,
	/// except that the returned pointer shall not be used to access an object.
	///
	/// # Errors
	///
	/// If the space cannot be allocated, a null pointer is returned and `errno`
	/// is set to `ENOMEM`.
	#[cfg_attr(prefixed, link_name = "_rjem_malloc")]
	pub fn malloc(size: size_t) -> *mut c_void;

	/// Allocates zero-initialized space for an array of `number` objects, each
	/// of whose size is `size`.
	///
	/// The result is identical to calling [`malloc`] with an argument of
	/// `number * size`, with the exception that the allocated memory is
	/// explicitly initialized to _zero_ bytes.
	///
	/// Note: zero-initialized memory need not be the same as the
	/// representation of floating-point zero or a null pointer constant.
	#[cfg_attr(prefixed, link_name = "_rjem_calloc")]
	pub fn calloc(number: size_t, size: size_t) -> *mut c_void;

	/// Allocates `size` bytes of memory at an address which is a multiple of
	/// `alignment` and is placed in `*ptr`.
	///
	/// If `size` is zero, then the value placed in `*ptr` is either null, or
	/// the behavior is as if the `size` were some nonzero value, except that
	/// the returned pointer shall not be used to access an object.
	///
	/// # Errors
	///
	/// On success, it returns zero. On error, the value of `errno` is _not_
	/// set, `*ptr` is not modified, and the return values can be:
	///
	/// - `EINVAL`: the `alignment` argument was not a power-of-two or was not a
	///   multiple of `mem::size_of::<*const c_void>()`.
	/// - `ENOMEM`: there was insufficient memory to fulfill the allocation
	///   request.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` is null.
	#[cfg_attr(prefixed, link_name = "_rjem_posix_memalign")]
	pub fn posix_memalign(ptr: *mut *mut c_void, alignment: size_t, size: size_t) -> c_int;

	/// Allocates `size` bytes of memory at an address which is a multiple of
	/// `alignment`.
	///
	/// If the `size` of the space requested is zero, either a null pointer is
	/// returned, or the behavior is as if the `size` were some nonzero value,
	/// except that the returned pointer shall not be used to access an object.
	///
	/// # Errors
	///
	/// Returns null if the request fails.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `alignment` is not a power-of-two
	/// * `size` is not an integral multiple of `alignment`
	#[cfg_attr(prefixed, link_name = "_rjem_aligned_alloc")]
	pub fn aligned_alloc(alignment: size_t, size: size_t) -> *mut c_void;

	/// Resizes the previously-allocated memory region referenced by `ptr` to
	/// `size` bytes.
	///
	/// Deallocates the old object pointed to by `ptr` and returns a pointer to
	/// a new object that has the size specified by `size`. The contents of the
	/// new object are the same as that of the old object prior to deallocation,
	/// up to the lesser of the new and old sizes.
	///
	/// The memory in the new object beyond the size of the old object is
	/// uninitialized.
	///
	/// The returned pointer to a new object may have the same value as a
	/// pointer to the old object, but [`realloc`] may move the memory
	/// allocation, resulting in a different return value than `ptr`.
	///
	/// If `ptr` is null, [`realloc`] behaves identically to [`malloc`] for the
	/// specified size.
	///
	/// If the size of the space requested is zero, the behavior is
	/// implementation-defined: either a null pointer is returned, or the
	/// behavior is as if the size were some nonzero value, except that the
	/// returned pointer shall not be used to access an object.
	///
	/// # Errors
	///
	/// If memory for the new object cannot be allocated, the old object is not
	/// deallocated, its value is unchanged, [`realloc`] returns null, and
	/// `errno` is set to `ENOMEM`.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` does not match a pointer previously returned by the memory
	///   allocation functions of this crate, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_realloc")]
	pub fn realloc(ptr: *mut c_void, size: size_t) -> *mut c_void;

	/// Deallocates previously-allocated memory region referenced by `ptr`.
	///
	/// This makes the space available for future allocations.
	///
	/// If `ptr` is null, no action occurs.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_free")]
	pub fn free(ptr: *mut c_void);

	/// Deallocates the previously-allocated memory region referenced by `ptr`,
	/// passing its allocation size as an optimization.
	///
	/// This is the ISO/IEC 9899:2024 (“ISO C23”) extension of [`free`] with a
	/// `size` parameter: supplying the size the region was requested with lets
	/// `jemalloc` skip work [`free`] must otherwise do to recover it. It is
	/// equivalent to [`sdallocx`](crate::sdallocx) with empty `flags`.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` is null. C23 specifies a null `ptr` as a no-op, but `jemalloc`
	///   forwards straight to [`sdallocx`](crate::sdallocx), which never treats
	///   null as one; the debug assert sits on its slow path, so the failure
	///   can also be a deferred crash rather than an immediate abort.
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate,
	/// * the memory region referenced by `ptr` has been deallocated,
	/// * `size` is not the size that was requested when `ptr` was allocated, or
	/// * `ptr` was allocated with an explicitly requested alignment, whether by
	///   [`aligned_alloc`], [`posix_memalign`], or [`mallocx`](crate::mallocx)
	///   with an alignment flag. Use [`free_aligned_sized`] or [`free`] for
	///   those.
	#[cfg_attr(prefixed, link_name = "_rjem_free_sized")]
	pub fn free_sized(ptr: *mut c_void, size: size_t);

	/// Deallocates the previously-allocated memory region referenced by `ptr`,
	/// passing both its allocation size and alignment as an optimization.
	///
	/// This is the ISO/IEC 9899:2024 (“ISO C23”) counterpart of [`free_sized`]
	/// for a region allocated with an explicitly requested alignment. It is
	/// equivalent to [`sdallocx`](crate::sdallocx) with
	/// `MALLOCX_ALIGN(alignment)`.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` is null, exactly as for [`free_sized`],
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate,
	/// * the memory region referenced by `ptr` has been deallocated, or
	/// * `size` and `alignment` are not the size and alignment that were
	///   requested when `ptr` was allocated.
	#[cfg_attr(prefixed, link_name = "_rjem_free_aligned_sized")]
	pub fn free_aligned_sized(ptr: *mut c_void, alignment: size_t, size: size_t);
}

//! The allocation entry points taking a `flags` argument, and their queries.
//!
//! Each allocating call takes a bitwise or of the `MALLOCX_*` bits where its
//! ISO C counterpart would use the implicit defaults, which is how a caller
//! names an alignment, an arena, or a thread cache for one call. The three
//! queries allocate nothing: `nallocx` reports the size class a request would
//! land in, `sallocx` and `malloc_usable_size` the one a pointer already
//! occupies.

use libc::{c_int, c_void, size_t};

unsafe extern "C" {
	/// Allocates at least `size` bytes of memory according to `flags`.
	///
	/// It returns a pointer to the start (lowest byte address) of the allocated
	/// space. This pointer is suitably aligned so that it may be assigned to a
	/// pointer to any type of object and then used to access such an object in
	/// the space allocated until the space is explicitly deallocated. Each
	/// yielded pointer points to an object disjoint from any other object.
	///
	/// # Errors
	///
	/// On success it returns a non-null pointer. A null pointer return value
	/// indicates that insufficient contiguous memory was available to service
	/// the allocation request.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if `size == 0`.
	#[cfg_attr(prefixed, link_name = "_rjem_mallocx")]
	pub fn mallocx(size: size_t, flags: c_int) -> *mut c_void;

	/// Resizes the previously-allocated memory region referenced by `ptr` to be
	/// at least `size` bytes.
	///
	/// Deallocates the old object pointed to by `ptr` and returns a pointer to
	/// a new object that has the size specified by `size`. The contents of the
	/// new object are the same as that of the old object prior to deallocation,
	/// up to the lesser of the new and old sizes.
	///
	/// The memory in the new object beyond the size of the old object is
	/// obtained according to `flags` (it might be uninitialized).
	///
	/// The returned pointer to a new object may have the same value as a
	/// pointer to the old object, but [`rallocx`] may move the memory
	/// allocation, resulting in a different return value than `ptr`.
	///
	/// # Errors
	///
	/// On success it returns a non-null pointer. A null pointer return value
	/// indicates that insufficient contiguous memory was available to service
	/// the allocation request. In this case, the old object is not
	/// deallocated, and its value is unchanged.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `size == 0`, or
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_rallocx")]
	pub fn rallocx(ptr: *mut c_void, size: size_t, flags: c_int) -> *mut c_void;

	/// Resizes the previously-allocated memory region referenced by `ptr` _in
	/// place_ to be at least `size` bytes, returning the real size of the
	/// allocation.
	///
	/// Deallocates the old object pointed to by `ptr` and sets `ptr` to a new
	/// object that has the size returned; the old and new objects share the
	/// same base address. The contents of the new object are the same as that
	/// of the old object prior to deallocation, up to the lesser of the new
	/// and old sizes.
	///
	/// If `extra` is non-zero, an attempt is made to resize the allocation to
	/// be at least `size + extra` bytes. Inability to allocate the `extra`
	/// bytes will not by itself result in failure to resize.
	///
	/// The memory in the new object beyond the size of the old object is
	/// obtained according to `flags` (it might be uninitialized).
	///
	/// # Errors
	///
	/// If the allocation cannot be adequately grown in place up to `size`, the
	/// size returned is smaller than `size`.
	///
	/// Note:
	///
	/// * the size value returned can be larger than the size requested during
	///   allocation
	/// * when shrinking an allocation, use the size returned to determine
	///   whether the allocation was shrunk sufficiently or not.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `size == 0`, or
	/// * `size + extra > size_t::MAX`, or
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_xallocx")]
	pub fn xallocx(ptr: *mut c_void, size: size_t, extra: size_t, flags: c_int) -> size_t;

	/// Returns the real size of the previously-allocated memory region
	/// referenced by `ptr`.
	///
	/// The value may be larger than the size requested on allocation.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_sallocx")]
	pub fn sallocx(ptr: *const c_void, flags: c_int) -> size_t;

	/// Deallocates previously-allocated memory region referenced by `ptr`.
	///
	/// This makes the space available for future allocations.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * `ptr` is null, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_dallocx")]
	pub fn dallocx(ptr: *mut c_void, flags: c_int);

	/// Deallocates previously-allocated memory region referenced by `ptr` with
	/// `size` hint.
	///
	/// This makes the space available for future allocations.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `size` is not in range `[req_size, alloc_size]`, where `req_size` is
	///   the size requested when performing the allocation, and `alloc_size` is
	///   the allocation size returned by [`nallocx`], [`sallocx`], or
	///   [`xallocx`],
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * `ptr` is null, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_sdallocx")]
	pub fn sdallocx(ptr: *mut c_void, size: size_t, flags: c_int);

	/// Returns the real size of the allocation that would result from a
	/// [`mallocx`] function call with the same arguments.
	///
	/// # Errors
	///
	/// If the inputs exceed the maximum supported size class and/or alignment
	/// it returns zero.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if `size == 0`.
	#[cfg_attr(prefixed, link_name = "_rjem_nallocx")]
	pub fn nallocx(size: size_t, flags: c_int) -> size_t;

	/// Returns the real size of the previously-allocated memory region
	/// referenced by `ptr`.
	///
	/// The value may be larger than the size requested on allocation.
	///
	/// Although the excess bytes can be overwritten by the application without
	/// ill effects, this is not good programming practice: the number of excess
	/// bytes in an allocation depends on the underlying implementation.
	///
	/// The main use of this function is for debugging and introspection.
	///
	/// # Errors
	///
	/// If `ptr` is null, 0 is returned.
	///
	/// # Safety
	///
	/// The behavior is _undefined_ if:
	///
	/// * `ptr` does not match a pointer earlier returned by the memory
	///   allocation functions of this crate, or
	/// * the memory region referenced by `ptr` has been deallocated.
	#[cfg_attr(prefixed, link_name = "_rjem_malloc_usable_size")]
	pub fn malloc_usable_size(ptr: *const c_void) -> size_t;
}

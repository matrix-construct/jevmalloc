// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The [`GlobalAlloc`] implementation for [`Jemalloc`].
//!
//! Each operation invokes its optional observation hook before normalizing the
//! caller's layout. Allocation then uses jemalloc's extended API, omitting the
//! alignment flag only when [`layout_flags`] proves the size class implies it.

pub mod hook;
pub mod layout;

use libc::{c_int, c_void};

use self::layout::{adjust_layout, layout_flags};
use super::{
	Jemalloc, ffi,
	ffi::MALLOCX_ZERO,
	std::{
		alloc::{GlobalAlloc, Layout},
		cmp,
		hint::assert_unchecked,
	},
};

/// Routes Rust's global allocation operations through jemalloc.
///
/// Each entry point adapts a nonzero Rust layout to the platform allocation
/// quantum before calling jemalloc's extended API. Optional hooks observe the
/// original arguments before that adaptation.
unsafe impl GlobalAlloc for Jemalloc {
	/// Allocates a block suitable for `layout`.
	///
	/// The allocation hook observes the original layout before normalization.
	/// jemalloc uses a null pointer to report failure, which is the failure
	/// return this method's contract requires, and is passed through
	/// unexamined.
	///
	/// # Safety
	///
	/// The caller must uphold [`GlobalAlloc::alloc`]'s contract, including its
	/// nonzero-size requirement.
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::ALLOC {
				hook(layout);
			}

			let layout = adjust_layout(layout);
			let flags = layout_flags(layout);
			let ptr = ffi::mallocx(layout.size(), flags);

			debug_validate(ptr, layout, flags);

			ptr.cast::<u8>()
		}
	}

	/// Allocates a zero-initialized block suitable for `layout`.
	///
	/// The allocation hook observes the original layout before normalization.
	/// jemalloc uses a null pointer to report failure, which is the failure
	/// return this method's contract requires, and is passed through
	/// unexamined.
	///
	/// # Safety
	///
	/// The caller must uphold [`GlobalAlloc::alloc_zeroed`]'s contract,
	/// including its nonzero-size requirement.
	unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::ALLOC_ZEROED {
				hook(layout);
			}

			let layout = adjust_layout(layout);
			let flags = layout_flags(layout) | MALLOCX_ZERO;
			let ptr = ffi::mallocx(layout.size(), flags);

			debug_validate(ptr, layout, flags);

			ptr.cast::<u8>()
		}
	}

	/// Resizes an existing allocation to `new_size` bytes.
	///
	/// The reallocation hook receives the original layout, pointer, and
	/// requested size before normalization. A null return leaves the original
	/// allocation live and owned by the caller, as the contract requires.
	///
	/// # Safety
	///
	/// The caller must uphold [`GlobalAlloc::realloc`]'s contract. In
	/// particular, `ptr` and `layout` must describe a live allocation from
	/// this allocator, and `new_size` must be nonzero and valid for
	/// `layout.align()`.
	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::REALLOC {
				hook(layout, ptr, new_size);
			}

			let layout = Layout::from_size_align_unchecked(new_size, layout.align());
			let layout = adjust_layout(layout);
			let flags = layout_flags(layout);
			let ptr = ffi::rallocx(ptr.cast::<c_void>(), layout.size(), flags);

			debug_validate(ptr, layout, flags);

			ptr.cast::<u8>()
		}
	}

	/// Releases an allocation described by `ptr` and `layout`.
	///
	/// The deallocation hook observes the original pointer and layout before
	/// normalization. jemalloc receives the normalized size as a deallocation
	/// hint together with the matching alignment flags.
	///
	/// # Safety
	///
	/// The caller must uphold [`GlobalAlloc::dealloc`]'s contract. The pointer
	/// must be non-null and must identify a live allocation created by this
	/// allocator for the supplied layout.
	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::DEALLOC {
				hook(layout, ptr);
			}

			assert_unchecked(!ptr.is_null());
			let ptr = ptr.cast::<c_void>();
			let layout = adjust_layout(layout);
			let flags = layout_flags(layout);

			debug_validate(ptr, layout, flags);
			ffi::sdallocx(ptr, layout.size(), flags);
		}
	}
}

/// Validates a served allocation against the request that produced it.
///
/// A null pointer is jemalloc's failure signal rather than an allocation, so
/// the helper returns without inspecting it. `sallocx` requires a live non-null
/// pointer, and a release jemalloc reads its radix tree for every supplied key.
///
/// # Safety
///
/// A non-null `ptr` must identify a live allocation obtained with `flags`.
#[inline]
#[track_caller]
unsafe fn debug_validate(ptr: *const c_void, layout: Layout, flags: c_int) {
	if ptr.is_null() {
		return;
	}

	unsafe {
		debug_assert!(
			ffi::nallocx(layout.size(), flags) >= layout.size(),
			"nallocx() rejected a request jemalloc served"
		);

		debug_assert!(ptr.addr().is_multiple_of(layout.align()), "alignment mismatch");
		debug_assert!(ffi::sallocx(ptr, flags) >= layout.size(), "sallocx() size mismatch");
	}
}

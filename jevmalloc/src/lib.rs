// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Bindings for jemalloc as an allocator
//!
//! This crate provides bindings to jemalloc as a memory allocator for Rust.
//! This crate mainly exports, one type, `Jemalloc`, which implements the
//! `GlobalAlloc` trait and optionally the `Alloc` trait,
//! and is suitable both as a memory allocator and as a global allocator.

#![no_std]

pub mod ctl;
mod global_alloc;

#[cfg(not(feature = "use_std"))]
use core as std;

#[cfg(feature = "use_std")]
pub(crate) use ::std;

/// Raw bindings to jemalloc
pub mod ffi {
	pub use jevmalloc_sys::*;
}

use core::{alloc::Layout, cmp, hint::assert_unchecked};

use libc::c_void;

pub use self::global_alloc::hook;
pub(crate) use crate::std::{fmt, num, result};

/// Handle to the jemalloc allocator
///
/// This type implements the `GlobalAllocAlloc` trait, allowing usage a global
/// allocator.
///
/// When the `api` feature of this crate is enabled, it also implements the
/// `Allocator` trait, allowing usage in collections.
#[derive(Debug)]
pub struct Jemalloc;

/// This constant equals _Alignof(max_align_t) and is platform-specific. It
/// contains the _maximum_ alignment that the memory allocations returned by the
/// C standard library memory allocation APIs (e.g. `malloc`) are guaranteed to
/// have.
///
/// The memory allocation APIs are required to return memory that can fit any
/// object whose fundamental aligment is <= _Alignof(max_align_t).
///
/// In C, there are no ZSTs, and the size of all types is a multiple of their
/// alignment (size >= align). So for allocations with size <=
/// _Alignof(max_align_t), the malloc-APIs return memory whose alignment is
/// either the requested size if its a power-of-two, or the next smaller
/// power-of-two.
#[cfg(any(
	target_arch = "arm",
	target_arch = "mips",
	target_arch = "powerpc"
))]
pub const QUANTUM: usize = 8;
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
pub const QUANTUM: usize = 16;

/// Adjust the layout's size and alignment based on platform requirements prior
/// to calls into jemalloc.
///
/// # Safety
///
/// This function only makes certain limited and efficient adjustments to the
/// input layout. It is not a general sanitizer. The input layout must still
/// construct a valid `Layout` which would not `Result` in `Err`, as the
/// construction here is unchecked.
#[inline]
#[must_use]
pub unsafe fn adjust_layout(layout: Layout) -> Layout {
	unsafe {
		assert_unchecked(layout.align() > 0);
		let align = cmp::max(layout.align(), QUANTUM);
		debug_assert!(align >= size_of::<c_void>(), "alignment too small");
		debug_assert!(align.is_power_of_two(), "alignment not a pow2");

		assert_unchecked(layout.size() > 0);
		let size = cmp::max(layout.size(), QUANTUM);
		debug_assert!(size >= size_of::<c_void>(), "size too small");
		debug_assert!(size >= align, "allocating a fragment");

		Layout::from_size_align_unchecked(size, align)
	}
}

/// Return the usable size of the allocation pointed to by ptr.
///
/// The return value may be larger than the size that was requested during
/// allocation. This function is not a mechanism for in-place `realloc()`;
/// rather it is provided solely as a tool for introspection purposes.
/// Any discrepancy between the requested allocation size
/// and the size reported by this function should not be depended on,
/// since such behavior is entirely implementation-dependent.
///
/// # Safety
///
/// `ptr` must have been allocated by `Jemalloc` and must not have been freed
/// yet.
#[inline]
pub unsafe fn usable_size<T>(ptr: *const T) -> usize {
	unsafe { ffi::malloc_usable_size(ptr.cast::<c_void>()) }
}

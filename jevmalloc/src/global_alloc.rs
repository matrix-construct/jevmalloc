// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub mod hook;

use super::{
	GlobalAlloc, Jemalloc, Layout, assert_unchecked, c_void, ffi,
	ffi::MALLOCX_ZERO,
	layout::{adjust_layout, layout_flags},
	uintptr_t,
};

unsafe impl GlobalAlloc for Jemalloc {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::ALLOC {
				hook(layout);
			}

			let layout = adjust_layout(layout);
			let flags = layout_flags(layout);
			debug_assert!(
				ffi::nallocx(layout.size(), flags) >= layout.size(),
				"alloc: nallocx() reported failure"
			);

			let ptr = ffi::mallocx(layout.size(), flags);
			debug_assert!(
				(ptr as uintptr_t).is_multiple_of(layout.align()),
				"alloc: alignment mismatch"
			);

			debug_assert!(
				ffi::sallocx(ptr, flags) >= layout.size(),
				"alloc: sallocx() size mismatch"
			);

			ptr.cast::<u8>()
		}
	}

	unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::ALLOC_ZEROED {
				hook(layout);
			}

			let layout = adjust_layout(layout);
			let flags = layout_flags(layout) | MALLOCX_ZERO;
			debug_assert!(
				ffi::nallocx(layout.size(), flags) >= layout.size(),
				"alloc_zeroed: nallocx() reported failure"
			);

			let ptr = ffi::mallocx(layout.size(), flags);
			debug_assert!(
				(ptr as uintptr_t).is_multiple_of(layout.align()),
				"alloc_zeroed: alignment mismatch"
			);

			debug_assert!(
				ffi::sallocx(ptr, flags) >= layout.size(),
				"alloc_zeroed: sallocx() size mismatch"
			);

			ptr.cast::<u8>()
		}
	}

	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::REALLOC {
				hook(layout, ptr, new_size);
			}

			let layout = Layout::from_size_align_unchecked(new_size, layout.align());
			let layout = adjust_layout(layout);
			let flags = layout_flags(layout);
			debug_assert!(
				ffi::nallocx(layout.size(), flags) >= layout.size(),
				"realloc: nallocx() reported failure"
			);

			let ptr = ffi::rallocx(ptr.cast::<c_void>(), layout.size(), flags);
			debug_assert!(
				(ptr as uintptr_t).is_multiple_of(layout.align()),
				"realloc: alignment mismatch"
			);

			debug_assert!(
				ffi::sallocx(ptr, flags) >= layout.size(),
				"realloc: sallocx() size mismatch"
			);

			ptr.cast::<u8>()
		}
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		unsafe {
			#[cfg(feature = "global_hooks")]
			if let Some(hook) = hook::DEALLOC {
				hook(layout, ptr);
			}

			assert_unchecked(!ptr.is_null());
			let ptr = ptr.cast::<c_void>();
			let layout = adjust_layout(layout);
			debug_assert!(
				(ptr as uintptr_t).is_multiple_of(layout.align()),
				"dealloc: alignment mismatch"
			);

			let flags = layout_flags(layout);
			debug_assert!(
				ffi::sallocx(ptr, flags) >= layout.size(),
				"dealloc: sallocx() size mismatch"
			);

			ffi::sdallocx(ptr, layout.size(), flags);
		}
	}
}

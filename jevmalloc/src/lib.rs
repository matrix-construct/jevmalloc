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
//! It mainly exports one type, `Jemalloc`, which implements the `GlobalAlloc`
//! trait and is suitable both as a memory allocator and as a global allocator.
//! It also re-exports the raw C bindings as [`ffi`], and wraps jemalloc's
//! control and introspection interface in [`ctl`].

#![no_std]

/// Raw bindings to jemalloc
///
/// Everything `jevmalloc-sys` exports, re-exported verbatim: the C entry
/// points, the `MALLOCX_*` flag helpers, and the FFI types.
pub mod ffi {
	pub use ::jevmalloc_sys::*;
}

pub mod ctl;
mod global_alloc;
mod layout;

use ::core as std;
use ::libc::{self, c_void, uintptr_t};

use self::std::{
	alloc::{GlobalAlloc, Layout},
	cmp,
	hint::assert_unchecked,
};
pub use self::{
	global_alloc::hook,
	layout::{QUANTUM, adjust_layout, layout_flags, usable_size},
};

/// Handle to the jemalloc allocator
///
/// It implements [`GlobalAlloc`](core::alloc::GlobalAlloc); install it as the
/// `#[global_allocator]` to route every Rust allocation through jemalloc.
#[derive(Debug)]
pub struct Jemalloc;

#[cfg(test)]
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A Rust global allocator backed by jemalloc.
//!
//! [`Jemalloc`] implements [`GlobalAlloc`] and can service the process-wide
//! `#[global_allocator]` slot. The [`ctl`] module wraps jemalloc's control and
//! introspection API, while [`ffi`] re-exports the underlying C bindings.

#![no_std]

/// Re-exports the raw jemalloc bindings.
///
/// The module exposes every item from `jevmalloc-sys`, including C entry
/// points, `MALLOCX_*` flag helpers, and foreign-function types. Callers must
/// uphold the safety contracts documented by that crate.
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

/// Selects jemalloc as a Rust global allocator.
///
/// Install this unit type in the `#[global_allocator]` slot to route Rust
/// allocations through jemalloc. Its [`GlobalAlloc`] implementation uses the
/// extended allocation API exported by [`ffi`].
#[derive(Debug)]
pub struct Jemalloc;

/// Installs jemalloc for the crate's unit tests.
///
/// The test allocator exercises the same process-wide entry points exposed to
/// downstream users. It is compiled only for this crate's unit-test target.
#[cfg(test)]
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Callbacks run on the way into jemalloc, one slot per allocator entry point.
//!
//! Each slot is consulted only when `feature = global_hooks` is enabled, and
//! holds `None` until an application installs one.

use super::Layout;

/// When `feature = global_hooks` enabled, called prior to entering
/// jemalloc.
pub static mut ALLOC: Option<fn(Layout)> = None;

/// When `feature = global_hooks` enabled, called prior to entering
/// jemalloc.
pub static mut ALLOC_ZEROED: Option<fn(Layout)> = None;

/// When `feature = global_hooks` enabled, called prior to entering
/// jemalloc.
pub static mut REALLOC: Option<fn(Layout, *const u8, usize)> = None;

/// When `feature = global_hooks` enabled, called prior to entering
/// jemalloc.
pub static mut DEALLOC: Option<fn(Layout, *const u8)> = None;

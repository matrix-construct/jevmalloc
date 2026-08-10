// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Observation hooks for jemalloc-backed global allocation operations.
//!
//! Each slot is consulted only with the `global_hooks` feature enabled and is
//! empty until an application installs a callback. Hooks run before layout
//! normalization and can execute concurrently or reentrantly. Allocating from
//! a callback invokes the same hook again unless it controls recursion.

use super::Layout;

/// Holds the callback invoked before an allocation.
///
/// The callback receives the caller's original layout. Install or replace it
/// only while no allocator operation can read the slot. Concurrent callback
/// invocations are valid after installation, but a conflicting unsynchronized
/// write to this mutable static is a data race.
pub static mut ALLOC: Option<fn(Layout)> = None;

/// Holds the callback invoked before a zeroed allocation.
///
/// The callback receives the caller's original layout. Install or replace it
/// only while no allocator operation can read the slot. Concurrent callback
/// invocations are valid after installation, but a conflicting unsynchronized
/// write to this mutable static is a data race.
pub static mut ALLOC_ZEROED: Option<fn(Layout)> = None;

/// Holds the callback invoked before a reallocation.
///
/// The callback receives the original layout and pointer followed by the new
/// requested size. Install or replace it only while no allocator operation can
/// read the slot. Concurrent callback invocations are valid after installation,
/// but a conflicting unsynchronized write to this mutable static is a data
/// race.
pub static mut REALLOC: Option<fn(Layout, *const u8, usize)> = None;

/// Holds the callback invoked before a deallocation.
///
/// The callback receives the caller's original layout and allocation pointer.
/// Install or replace it only while no allocator operation can read the slot,
/// because a conflicting unsynchronized write is a data race. Concurrent
/// callback invocations are valid after installation.
pub static mut DEALLOC: Option<fn(Layout, *const u8)> = None;

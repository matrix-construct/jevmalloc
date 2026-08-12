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
//! `#[global_allocator]` slot. Typed allocator operations are grouped by scope
//! in [`Arena`], [`arenas`], [`config`], [`opt`], [`stats`], and [`thread`].
//! The [`ctl`] module exposes MIB-based control-interface primitives, while
//! [`ffi`] re-exports the underlying C bindings.
//!
//! ```
//! #[global_allocator]
//! static ALLOC: jevmalloc::Jemalloc = jevmalloc::Jemalloc;
//!
//! # fn main() -> Result<(), jevmalloc::ctl::Error> {
//! let quantum = jevmalloc::arenas::quantum()?;
//! let epoch = jevmalloc::stats::refresh_epoch()?;
//! assert!(quantum > 0);
//! assert!(epoch > 0);
//! # Ok(())
//! # }
//! ```
//!
//! [`GlobalAlloc`]: core::alloc::GlobalAlloc

#![no_std]

pub mod arena;
pub mod arenas;
pub mod config;
pub mod ctl;
pub mod global;
pub mod opt;
#[cfg(feature = "profiling")]
pub mod profiling;
pub mod stats;
pub mod thread;

/// Re-exports the raw jemalloc bindings.
///
/// The `jevmalloc-sys` crate exposes the C entry points, the `MALLOCX_*` flag
/// helpers, and the foreign-function types. Callers must uphold the safety
/// contracts documented there.
pub use ::jevmalloc_sys as ffi;
pub use arena::{
	ARENA_INDEX_LIMIT, ARENA_NAME_LEN, Arena, ArenaDestroyError, ArenaName, Dss, ExtentHooks,
};

pub use self::ctl::{Error, Result};
/// Re-exports the allocator layout utilities.
pub use self::global::layout::*;
#[cfg(feature = "profiling")]
pub use self::profiling::{
	is_prof_enabled, prof_dump, prof_enable, prof_gdump, prof_interval, prof_reset,
};
#[cfg(feature = "stats")]
pub use self::stats::stats_reset;

/// Selects jemalloc as a Rust global allocator.
///
/// Install this unit type in the `#[global_allocator]` slot to route Rust
/// allocations through jemalloc. Its [`GlobalAlloc`] implementation uses the
/// extended allocation API exported by [`ffi`].
///
/// [`GlobalAlloc`]: core::alloc::GlobalAlloc
#[derive(Clone, Copy, Debug)]
pub struct Jemalloc;

/// Installs jemalloc for the crate's unit tests.
///
/// The test allocator exercises the same process-wide entry points exposed to
/// downstream users. It is compiled only for this crate's unit-test target.
#[cfg(test)]
#[global_allocator]
static ALLOCATOR: Jemalloc = Jemalloc;

/// Enables or disables jemalloc's background purge workers.
///
/// Enabling creates workers on demand. Disabling waits for existing workers to
/// terminate before returning. The previous setting is returned. If jemalloc
/// reports a worker-management failure, the requested setting can still have
/// taken effect. A child process starts with workers disabled after `fork`,
/// regardless of the parent setting. The control exists only on selected
/// pthread-based platforms.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the read or update.
pub fn background_thread_enable(enable: bool) -> Result<bool> {
	let key = self::ctl::key::background_thread()?;
	self::ctl::value::xchg_bool(&key, enable)
}

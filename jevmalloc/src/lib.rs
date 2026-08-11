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
//! introspection API with a compact MIB-only surface, while [`ffi`] re-exports
//! the underlying C bindings.
//!
//! ```
//! #[global_allocator]
//! static ALLOC: jevmalloc::Jemalloc = jevmalloc::Jemalloc;
//!
//! # fn main() -> Result<(), jevmalloc::ctl::Error> {
//! let quantum = jevmalloc::ctl::quantum()?;
//! let epoch = jevmalloc::ctl::refresh_epoch()?;
//! assert!(quantum > 0);
//! assert!(epoch > 0);
//! # Ok(())
//! # }
//! ```
//!
//! [`GlobalAlloc`]: core::alloc::GlobalAlloc

#![no_std]

pub mod ctl;
pub mod global_alloc;

use ::core as std;
/// Re-exports the raw jemalloc bindings.
///
/// The `jevmalloc-sys` crate exposes the C entry points, the `MALLOCX_*` flag
/// helpers, and the foreign-function types. Callers must uphold the safety
/// contracts documented there.
pub use ::jevmalloc_sys as ffi;

/// Re-exports the allocator layout utilities.
pub use self::global_alloc::layout::*;

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
static ALLOC: Jemalloc = Jemalloc;

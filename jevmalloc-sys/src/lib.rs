//! Rust bindings to the `jemalloc` C library.
//!
//! `jemalloc` is a general purpose memory allocation, its documentation
//! can be found here:
//!
//! * [API documentation][jemalloc_docs]
//! * [Wiki][jemalloc_wiki] (design documents, presentations, profiling,
//!   debugging, tuning, ...)
//!
//! `jemalloc` exposes both a standard and a non-standard API.
//!
//! # Standard API
//!
//! The standard API includes: the [`malloc`], [`calloc`], [`realloc`], and
//! [`free`], which conform to to ISO/IEC 9899:1990 (“ISO C90”),
//! [`posix_memalign`] which conforms to conforms to POSIX.1-2016, and
//! [`aligned_alloc`].
//!
//! Note that these standard leave some details as _implementation defined_.
//! This docs document this behavior for `jemalloc`, but keep in mind that other
//! standard-conforming implementations of these functions in other allocators
//! might behave slightly different.
//!
//! # Non-Standard API
//!
//! The non-standard API includes: [`mallocx`], [`rallocx`], [`xallocx`],
//! [`sallocx`], [`dallocx`], [`sdallocx`], and [`nallocx`]. These functions all
//! have a `flags` argument that can be used to specify options. Use bitwise or
//! `|` to specify one or more of the following: [`MALLOCX_LG_ALIGN`],
//! [`MALLOCX_ALIGN`], [`MALLOCX_ZERO`], [`MALLOCX_TCACHE`],
//! [`MALLOCX_TCACHE_NONE`], and [`MALLOCX_ARENA`].
//!
//! # Environment variables
//!
//! The `MALLOC_CONF` environment variable affects the execution of the
//! allocation functions.
//!
//! For the documentation of the [`MALLCTL` namespace visit the jemalloc
//! documenation][jemalloc_mallctl].
//!
//! [jemalloc_docs]: http://jemalloc.net/jemalloc.3.html
//! [jemalloc_wiki]: https://github.com/jemalloc/jemalloc/wiki
//! [jemalloc_mallctl]: http://jemalloc.net/jemalloc.3.html#mallctl_namespace
#![no_std]
#![allow(non_snake_case, non_camel_case_types, unnecessary_safety_doc)]
#![deny(missing_docs, broken_intra_doc_links)]

mod control;
mod env;
mod extended;
mod extent;
mod flags;
mod standard;

pub use self::{control::*, env::*, extended::*, extent::*, flags::*, standard::*};

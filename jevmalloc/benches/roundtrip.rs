//! Measures allocation and deallocation round trips across allocator APIs.
//!
//! This target is gated by the `bench` configuration because it uses the
//! unstable `test` harness while the crate itself supports stable Rust. The
//! complete matrix contains 50 sizes, 6 alignments, and 12 round-trip methods,
//! so targeted runs should select a benchmark name. The repository README's
//! Benchmarks section gives the exact nightly invocation and filter syntax.

#![cfg(test)]
#![cfg(bench)]
#![cfg_attr(bench, feature(test))]

/// Unstable benchmark harness supplied by the Rust toolchain.
extern crate test;

use std::{
	alloc::{GlobalAlloc, Layout},
	ptr,
};

use ::test::Bencher;
use jevmalloc::{Jemalloc, ffi, layout_flags};

/// Routes benchmark-harness and `GlobalAlloc` traffic through jemalloc.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Generates all round-trip variants for one pair or a list of sizes.
///
/// The list form expands each size across power-of-two alignments from 1
/// through 32.
macro_rules! rt {
    ($size:expr, $align:expr) => {
        paste::paste! {
            /// Measures a raw `mallocx` and `sdallocx` round trip.
            #[bench]
            fn [<rt_mallocx_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = ffi::mallocx($size, flags);
                    test::black_box(ptr);
                    ffi::sdallocx(ptr, $size, flags);
                });
            }

            /// Measures a raw round trip with an intervening `nallocx` query.
            #[bench]
            fn [<rt_mallocx_nallocx_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = ffi::mallocx($size, flags);
                    test::black_box(ptr);
                    let rsz = ffi::nallocx($size, flags);
                    test::black_box(rsz);
                    ffi::sdallocx(ptr, rsz, flags);
                });
            }

            /// Measures `GlobalAlloc` with a checked layout construction.
            #[bench]
            fn [<rt_alloc_layout_checked_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align($size, $align).unwrap();
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);
                    Jemalloc.dealloc(ptr, layout);
                });
            }

            /// Measures `GlobalAlloc` with an unchecked layout construction.
            #[bench]
            fn [<rt_alloc_layout_unchecked_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align_unchecked($size, $align);
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);
                    Jemalloc.dealloc(ptr, layout);
                });
            }

            /// Measures querying usable size without using it for deallocation.
            ///
            /// This replaces the removed `alloc_excess` path by reading the
            /// usable size back through `sallocx`.
            #[bench]
            fn [<rt_alloc_sallocx_unused_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = ffi::mallocx($size, flags);
                    test::black_box(ptr);
                    let excess = ffi::sallocx(ptr, flags);
                    test::black_box(excess);
                    ffi::sdallocx(ptr, $size, flags);
                });
            }

            /// Measures querying and reusing the usable size for deallocation.
            #[bench]
            fn [<rt_alloc_sallocx_used_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = ffi::mallocx($size, flags);
                    test::black_box(ptr);
                    let excess = ffi::sallocx(ptr, flags);
                    test::black_box(excess);
                    ffi::sdallocx(ptr, excess, flags);
                });
            }

            /// Measures zeroed `mallocx` followed by sized deallocation.
            #[bench]
            fn [<rt_mallocx_zeroed_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = ffi::mallocx($size, flags | ffi::MALLOCX_ZERO);
                    test::black_box(ptr);
                    ffi::sdallocx(ptr, $size, flags);
                });
            }

            /// Measures `calloc` followed by sized deallocation.
            #[bench]
            fn [<rt_calloc_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    test::black_box(flags);
                    let ptr = ffi::calloc(1, $size);
                    test::black_box(ptr);
                    ffi::sdallocx(ptr, $size, 0);
                });
            }

            /// Measures the C23 `malloc` and `free_sized` round trip.
            ///
            /// jemalloc 5.3.1 forwards `free_sized` to `sdallocx`, making this
            /// benchmark sensitive to the entry point's call overhead.
            #[bench]
            fn [<rt_malloc_free_sized_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let ptr = ffi::malloc($size);
                    test::black_box(ptr);
                    ffi::free_sized(ptr, $size);
                });
            }

            /// Measures the C23 aligned allocation and deallocation round trip.
            ///
            /// jemalloc 5.3.1 forwards `free_aligned_sized` to `sdallocx`,
            /// making this benchmark sensitive to the entry point's overhead.
            #[bench]
            fn [<rt_mallocx_free_aligned_sized_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let ptr = ffi::mallocx($size, ffi::MALLOCX_ALIGN($align));
                    test::black_box(ptr);
                    ffi::free_aligned_sized(ptr, $align, $size);
                });
            }

            /// Measures reallocation implemented as allocate, copy, and free.
            #[bench]
            fn [<rt_realloc_naive_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align($size, $align).unwrap();
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);

                    // Implement the naive strategy with explicit allocation, copy, and free.
                    let new_layout = Layout::from_size_align(2 * $size, $align).unwrap();
                    let ptr = {
                        let new_ptr = Jemalloc.alloc(new_layout);
                        ptr::copy_nonoverlapping(ptr.cast_const(), new_ptr, layout.size());
                        Jemalloc.dealloc(ptr, layout);
                        new_ptr
                    };
                    test::black_box(ptr);

                    Jemalloc.dealloc(ptr, new_layout);
                });
            }

            /// Measures reallocation through `GlobalAlloc::realloc`.
            #[bench]
            fn [<rt_realloc_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align($size, $align).unwrap();
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);

                    let new_layout = Layout::from_size_align(2 * $size, $align).unwrap();
                    let ptr = Jemalloc.realloc(ptr, layout, new_layout.size());
                    test::black_box(ptr);

                    Jemalloc.dealloc(ptr, new_layout);
                });
            }
        }
    };
    ([$($size:expr),*]) => {
        $(
            rt!($size, 1);
            rt!($size, 2);
            rt!($size, 4);
            rt!($size, 8);
            rt!($size, 16);
            rt!($size, 32);
        )*
    }
}

/// Benchmarks selected power-of-two allocation sizes.
mod pow2 {
	use super::*;

	rt!([
		1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
		131072, 4194304
	]);
}

/// Benchmarks selected even decimal allocation sizes.
mod even {
	use super::*;

	rt!([10, 100, 1000, 10000, 100000, 1000000]);
}

/// Benchmarks odd sizes immediately below the selected decimal sizes.
mod odd {
	use super::*;
	rt!([9, 99, 999, 9999, 99999, 999999]);
}

/// Benchmarks irregular sizes centered on primes, plus the legacy 96-byte case.
mod primes {
	use super::*;
	rt!([
		3, 7, 13, 17, 31, 61, 96, 127, 257, 509, 1021, 2039, 4093, 8191, 16381, 32749, 65537,
		131071, 4194301
	]);
}

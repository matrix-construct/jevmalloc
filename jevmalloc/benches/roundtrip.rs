//! Benchmarks the cost of the different allocation functions by doing a
//! roundtrip (allocate, deallocate).
//!
//! Gated behind `--cfg bench` because it needs the unstable `test` harness, and
//! the crate itself builds on stable. Run it with:
//!
//! ```shell
//! RUSTFLAGS='--cfg bench' cargo +nightly bench -p jevmalloc
//! ```
//!
//! The matrix is 50 sizes x 6 alignments x 12 roundtrips, so a full run is very
//! long; pass a filter (`... -- rt_mallocx_size_4096`) for anything targeted.

#![cfg(bench)]
#![cfg_attr(bench, feature(test))]

extern crate test;

use std::{
	alloc::{GlobalAlloc, Layout},
	ptr,
};

use jevmalloc::{Jemalloc, layout_flags};
use test::Bencher;

#[global_allocator]
static A: Jemalloc = Jemalloc;

macro_rules! rt {
    ($size:expr, $align:expr) => {
        paste::paste! {
            #[bench]
            fn [<rt_mallocx_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = jemalloc::mallocx($size, flags);
                    test::black_box(ptr);
                    jemalloc::sdallocx(ptr, $size, flags);
                });
            }

            #[bench]
            fn [<rt_mallocx_nallocx_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = jemalloc::mallocx($size, flags);
                    test::black_box(ptr);
                    let rsz = jemalloc::nallocx($size, flags);
                    test::black_box(rsz);
                    jemalloc::sdallocx(ptr, rsz, flags);
                });
            }

            #[bench]
            fn [<rt_alloc_layout_checked_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align($size, $align).unwrap();
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);
                    Jemalloc.dealloc(ptr, layout);
                });
            }

            #[bench]
            fn [<rt_alloc_layout_unchecked_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align_unchecked($size, $align);
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);
                    Jemalloc.dealloc(ptr, layout);
                });
            }

            // Stands in for the removed `alloc_excess`: the usable size is now
            // read back with `sallocx` rather than returned by the allocation.
            #[bench]
            fn [<rt_alloc_sallocx_unused_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = jemalloc::mallocx($size, flags);
                    test::black_box(ptr);
                    let excess = jemalloc::sallocx(ptr, flags);
                    test::black_box(excess);
                    jemalloc::sdallocx(ptr, $size, flags);
                });
            }

            #[bench]
            fn [<rt_alloc_sallocx_used_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = jemalloc::mallocx($size, flags);
                    test::black_box(ptr);
                    let excess = jemalloc::sallocx(ptr, flags);
                    test::black_box(excess);
                    jemalloc::sdallocx(ptr, excess, flags);
                });
            }

            #[bench]
            fn [<rt_mallocx_zeroed_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    let ptr = jemalloc::mallocx($size, flags | jemalloc::MALLOCX_ZERO);
                    test::black_box(ptr);
                    jemalloc::sdallocx(ptr, $size, flags);
                });
            }

            #[bench]
            fn [<rt_calloc_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let flags = layout_flags(Layout::from_size_align($size, $align).unwrap());
                    test::black_box(flags);
                    let ptr = jemalloc::calloc(1, $size);
                    test::black_box(ptr);
                    jemalloc::sdallocx(ptr, $size, 0);
                });
            }

            // The ISO C23 sized deallocations, new in jemalloc 5.3.1. Both are
            // forwards onto `sdallocx`, so a difference here is call overhead.
            #[bench]
            fn [<rt_malloc_free_sized_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let ptr = jemalloc::malloc($size);
                    test::black_box(ptr);
                    jemalloc::free_sized(ptr, $size);
                });
            }

            #[bench]
            fn [<rt_mallocx_free_aligned_sized_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    use jevmalloc::ffi as jemalloc;
                    let ptr = jemalloc::mallocx($size, jemalloc::MALLOCX_ALIGN($align));
                    test::black_box(ptr);
                    jemalloc::free_aligned_sized(ptr, $align, $size);
                });
            }

            #[bench]
            fn [<rt_realloc_naive_size_ $size _align_ $align>](b: &mut Bencher) {
                b.iter(|| unsafe {
                    let layout = Layout::from_size_align($size, $align).unwrap();
                    let ptr = Jemalloc.alloc(layout);
                    test::black_box(ptr);

                    // naive realloc:
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

// Powers of two
mod pow2 {
	use super::*;

	rt!([
		1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
		131072, 4194304
	]);
}

mod even {
	use super::*;

	rt!([10, 100, 1000, 10000, 100000, 1000000]);
}

mod odd {
	use super::*;
	rt!([9, 99, 999, 9999, 99999, 999999]);
}

mod primes {
	use super::*;
	rt!([
		3, 7, 13, 17, 31, 61, 96, 127, 257, 509, 1021, 2039, 4093, 8191, 16381, 32749, 65537,
		131071, 4194301
	]);
}

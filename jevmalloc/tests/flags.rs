//! Exercises both layout flag branches across the `GlobalAlloc` surface.
//!
//! `layout_flags` drops the alignment bits when a size class already satisfies
//! the alignment, so the allocator has two paths: the zero flag word that
//! reaches jemalloc's thread-cache fast path, and the `MALLOCX_ALIGN` word that
//! does not. Every case is handed the word it will be allocated with, and
//! `for_each_case` fails the test if a walk did not visit both branches.

#![cfg(test)]

use core::{
	alloc::{GlobalAlloc, Layout},
	ffi::c_int,
	slice,
};

use jevmalloc::{
	Jemalloc, QUANTUM, adjust_layout, ctl,
	ffi::{MALLOCX_ALIGN, nallocx},
	layout_flags,
};

/// Routes allocations made by the test harness through jemalloc.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Spans requests below, at, and well above the allocator quantum.
///
/// The 65,536-byte case lies beyond the default thread cache, while the 1 MiB
/// case is served by an extent rather than a slab.
const SIZES: [usize; 10] = [1, 8, 16, 17, 64, 100, 512, 4096, 65536, 1 << 20];

/// Power-of-two alignments spanning both sides of [`QUANTUM`].
const ALIGNS: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

/// Spans fragment sizes below every explicit alignment in the focused matrix.
///
/// Each value remains below [`QUANTUM`], so normalization raises its size while
/// preserving the requested larger alignment.
const FRAGMENT_SIZES: [usize; 3] = [1, 7, 15];

/// Spans explicit alignments on both sides of a typical page boundary.
///
/// The largest case also exceeds jemalloc's default thread-cache ceiling, so
/// the round trip covers both slab and extent-backed allocations.
const FRAGMENT_ALIGNS: [usize; 5] = [32, 64, 128, 4096, 65536];

/// Gives the largest size any valid nonzero layout can carry.
///
/// The alignment-one layout is valid, but raising its alignment to [`QUANTUM`]
/// would make its rounded size exceed `isize::MAX`. jemalloc rejects the
/// original request as unrepresentable.
const MAX_LAYOUT_SIZE: usize = isize::MAX.unsigned_abs();

/// Gives a representable size that exceeds supported user address spaces.
///
/// jemalloc can compute its size class without being able to obtain a mapping,
/// which exercises the allocation-failure path after `nallocx` succeeds.
#[cfg(target_pointer_width = "64")]
const UNMAPPABLE: usize = 1 << 61;

/// Confirms that only alignments above the quantum retain alignment flags.
///
/// After [`adjust_layout`], every supported layout at or below [`QUANTUM`] is
/// already aligned by its size class.
#[test]
fn flag_word_follows_the_quantum() {
	for_each_case(|layout, flags| {
		assert_eq!(
			flags != 0,
			layout.align() > QUANTUM,
			"{layout:?} landed on the wrong branch with flags {flags}"
		);
	});
}

/// Visits each size and alignment pair with its computed flag word.
///
/// Every declared pair is a valid Rust layout, including requests whose
/// alignment exceeds their size. The walk fails the test if it did not visit
/// both flag branches.
///
/// # Panics
///
/// Panics if a declared pair cannot form a layout or if the matrix does not
/// exercise both flag branches.
fn for_each_case(mut case: impl FnMut(Layout, c_int)) {
	let (fast, aligned) = SIZES
		.iter()
		.copied()
		.flat_map(|size| {
			ALIGNS
				.iter()
				.copied()
				.map(move |align| (size, align))
		})
		.map(|(size, align)| {
			let layout = Layout::from_size_align(size, align).unwrap();
			// SAFETY: every matrix entry has a nonzero size.
			let adjusted = unsafe { adjust_layout(layout) };
			let flags = layout_flags(adjusted);

			case(layout, flags);
			flags
		})
		.fold((0_usize, 0_usize), |(fast, aligned), flags| {
			(fast + usize::from(flags == 0), aligned + usize::from(flags != 0))
		});

	assert!(fast > 0, "no case reached the fast path");
	assert!(aligned > 0, "no case carried an alignment");
}

/// Confirms that fragment layouts retain an explicit alignment flag.
///
/// The size class cannot imply an alignment larger than its requested size, so
/// each adjusted layout must carry the corresponding `MALLOCX_ALIGN` value.
#[test]
fn fragment_layouts_retain_alignment_flags() {
	for_each_fragment(|layout| {
		// SAFETY: every fragment layout has a nonzero size.
		let adjusted = unsafe { adjust_layout(layout) };
		assert_eq!(layout_flags(adjusted), MALLOCX_ALIGN(layout.align()));
	});
}

/// Visits every focused fragment layout.
///
/// Each declared pair is valid even though its requested alignment exceeds its
/// size.
///
/// # Panics
///
/// Panics if a declared pair cannot form a layout.
fn for_each_fragment(mut case: impl FnMut(Layout)) {
	for size in FRAGMENT_SIZES {
		for align in FRAGMENT_ALIGNS {
			case(Layout::from_size_align(size, align).unwrap());
		}
	}
}

/// Confirms that allocations remain aligned on both flag branches.
///
/// The test checks each returned pointer rather than inferring alignment from
/// the selected size class.
#[test]
fn allocations_are_aligned_on_both_branches() {
	for_each_case(|layout, flags| {
		// SAFETY: every matrix layout is valid and nonzero.
		let ptr = unsafe { Jemalloc.alloc(layout) };

		assert!(!ptr.is_null(), "{layout:?} flags {flags} failed to allocate");
		let aligned = ptr.addr().is_multiple_of(layout.align());

		// SAFETY: the live allocation contains at least `layout.size()` writable
		// bytes.
		unsafe { ptr.write_bytes(0xA5, layout.size()) };

		// SAFETY: `ptr` remains live and was created with this exact layout.
		unsafe { Jemalloc.dealloc(ptr, layout) };

		assert!(aligned, "{layout:?} flags {flags} came back underaligned");
	});
}

/// Confirms that fragments remain aligned, resizable, and freeable.
///
/// The matrix crosses the page boundary and writes every requested byte before
/// and after growing each allocation.
#[test]
fn fragment_allocations_round_trip() {
	for_each_fragment(|layout| {
		// SAFETY: every fragment layout is valid and nonzero.
		let ptr = unsafe { Jemalloc.alloc(layout) };
		assert!(!ptr.is_null(), "{layout:?} failed to allocate");
		let allocated_aligned = ptr.addr().is_multiple_of(layout.align());

		// SAFETY: the live allocation contains at least `layout.size()` writable
		// bytes.
		unsafe { ptr.write_bytes(0xA5, layout.size()) };
		let size = layout.size() + 1;

		// SAFETY: `ptr` is live from this allocator, and `size` is nonzero and
		// valid for the original alignment.
		let grown = unsafe { Jemalloc.realloc(ptr, layout, size) };
		if grown.is_null() {
			// SAFETY: failed reallocation leaves the original allocation live.
			unsafe { Jemalloc.dealloc(ptr, layout) };
			panic!("{layout:?} failed to grow");
		}

		let grown_aligned = grown.addr().is_multiple_of(layout.align());

		// SAFETY: successful reallocation returned at least `size` writable bytes.
		unsafe { grown.write_bytes(0x5A, size) };

		let layout = Layout::from_size_align(size, layout.align()).unwrap();

		// SAFETY: `grown` remains live and was created with this new layout.
		unsafe { Jemalloc.dealloc(grown, layout) };

		assert!(allocated_aligned, "{layout:?} was initially underaligned");
		assert!(grown_aligned, "{layout:?} grew underaligned");
	});
}

/// Confirms that zeroed allocations are aligned and initialized on both paths.
///
/// `MALLOCX_ZERO` makes the allocation word nonzero, so only deallocation can
/// take the fast path; alignment and zeroing must hold on both branches
/// regardless.
#[test]
fn zeroed_allocations_are_aligned_and_zero_on_both_branches() {
	for_each_case(|layout, flags| {
		// SAFETY: every matrix layout is valid and nonzero.
		let ptr = unsafe { Jemalloc.alloc_zeroed(layout) };
		assert!(!ptr.is_null(), "{layout:?} flags {flags} failed to allocate");
		let aligned = ptr.addr().is_multiple_of(layout.align());

		// SAFETY: the live allocation contains `layout.size()` initialized bytes.
		let bytes = unsafe { slice::from_raw_parts(ptr, layout.size()) };
		let zeroed = bytes.iter().all(|byte| *byte == 0);

		// SAFETY: `ptr` remains live and was created with this exact layout.
		unsafe { Jemalloc.dealloc(ptr, layout) };

		assert!(aligned, "{layout:?} flags {flags} came back underaligned");
		assert!(zeroed, "{layout:?} flags {flags} was not zeroed");
	});
}

/// Confirms that growing and shrinking preserve alignment on both flag paths.
///
/// The branch is determined by alignment, which `realloc` preserves. Shrinking
/// below that alignment exercises the fragment-layout path.
#[test]
fn reallocations_are_aligned_on_both_branches() {
	for_each_case(|layout, flags| {
		for size in [layout.size() * 2, layout.size() / 2 + 1] {
			let after = Layout::from_size_align(size, layout.align()).unwrap();

			// SAFETY: every matrix layout is valid and nonzero.
			let ptr = unsafe { Jemalloc.alloc(layout) };
			assert!(!ptr.is_null(), "{layout:?} flags {flags} failed to allocate");

			// SAFETY: `ptr` is live from this allocator, and `size` is nonzero and
			// valid for the original alignment.
			let resized = unsafe { Jemalloc.realloc(ptr, layout, size) };
			if resized.is_null() {
				// SAFETY: failed reallocation leaves the original allocation live.
				unsafe { Jemalloc.dealloc(ptr, layout) };
				panic!("{layout:?} -> {size} flags {flags} failed to reallocate");
			}

			let aligned = resized.addr().is_multiple_of(layout.align());

			// SAFETY: successful reallocation returned at least `size` writable
			// bytes.
			unsafe { resized.write_bytes(0x5A, size) };

			// SAFETY: `resized` remains live and was created with `after`.
			unsafe { Jemalloc.dealloc(resized, after) };

			assert!(aligned, "{layout:?} -> {size} flags {flags} came back underaligned");
		}
	});
}

/// Confirms that unrepresentable requests return jemalloc's failure signal.
///
/// The adjusted request is a valid Rust layout for which `nallocx` returns
/// zero. All allocating entry points must return null without inspecting it.
#[test]
fn unrepresentable_requests_return_null() {
	let layout = Layout::from_size_align(MAX_LAYOUT_SIZE, 1).unwrap();

	// SAFETY: the maximum-size layout remains nonzero.
	let adjusted = unsafe { adjust_layout(layout) };
	let flags = layout_flags(adjusted);
	assert_eq!(adjusted, layout);

	// SAFETY: the adjusted size is nonzero and `flags` is valid for its layout.
	assert_eq!(unsafe { nallocx(adjusted.size(), flags) }, 0);
	assert_failure_returns_null(layout);
}

/// Exercises every allocating entry point with a request that must fail.
///
/// A failed reallocation must leave its original allocation live, writable,
/// and owned by the caller.
///
/// # Panics
///
/// Panics if an allocation unexpectedly succeeds or the original allocation
/// does not survive a failed reallocation.
fn assert_failure_returns_null(failed: Layout) {
	let size = failed.size();
	let original = Layout::from_size_align(QUANTUM, failed.align()).unwrap();

	// SAFETY: `failed` is valid and nonzero.
	let allocated = unsafe { Jemalloc.alloc(failed) };
	if !allocated.is_null() {
		// SAFETY: the unexpected success returned a live allocation for `failed`.
		unsafe { Jemalloc.dealloc(allocated, failed) };
	}
	assert!(allocated.is_null(), "alloc({size}) unexpectedly succeeded");

	// SAFETY: `failed` is valid and nonzero.
	let zeroed = unsafe { Jemalloc.alloc_zeroed(failed) };
	if !zeroed.is_null() {
		// SAFETY: the unexpected success returned a live allocation for `failed`.
		unsafe { Jemalloc.dealloc(zeroed, failed) };
	}
	assert!(zeroed.is_null(), "alloc_zeroed({size}) unexpectedly succeeded");

	// SAFETY: `original` is valid and nonzero.
	let ptr = unsafe { Jemalloc.alloc(original) };
	assert!(!ptr.is_null(), "setup allocation failed");

	// SAFETY: `ptr` identifies at least one writable byte in a live allocation.
	unsafe { ptr.write(0xA5) };

	// SAFETY: `ptr` is live from this allocator, and `failed` has the same
	// alignment and a valid nonzero size.
	let grown = unsafe { Jemalloc.realloc(ptr, original, size) };
	let preserved = if grown.is_null() {
		// SAFETY: failed reallocation leaves the initialized original live.
		let value = unsafe { ptr.read() };

		// SAFETY: `ptr` remains live and retains its original layout.
		unsafe { Jemalloc.dealloc(ptr, original) };
		Some(value)
	} else {
		// SAFETY: successful reallocation consumed `ptr` and returned a live
		// allocation described by `failed`.
		unsafe { Jemalloc.dealloc(grown, failed) };
		None
	};

	assert!(grown.is_null(), "realloc({size}) unexpectedly succeeded");
	assert_eq!(preserved, Some(0xA5), "failed realloc changed the original allocation");
}

/// Confirms that unmappable requests return jemalloc's failure signal.
///
/// `nallocx` accepts the request, but no mapping can satisfy it. All allocating
/// entry points must return null without inspecting it.
#[cfg(target_pointer_width = "64")]
#[test]
fn unmappable_requests_return_null() {
	let layout = Layout::from_size_align(UNMAPPABLE, QUANTUM).unwrap();

	// SAFETY: the unmappable test layout remains nonzero.
	let adjusted = unsafe { adjust_layout(layout) };
	let flags = layout_flags(adjusted);

	// SAFETY: the adjusted size is nonzero and `flags` is valid for its layout.
	assert_ne!(unsafe { nallocx(adjusted.size(), flags) }, 0);
	assert_failure_returns_null(layout);
}

/// Confirms that dropping quantum alignment preserves the selected size class.
///
/// The sweep covers every small class and the first large classes. At or above
/// the quantum, every selected class must already be quantum-aligned.
#[test]
fn dropping_the_alignment_keeps_the_size_class() {
	for size in QUANTUM..=65536 {
		// SAFETY: every loop size is nonzero and the zero flag word is valid.
		let plain = unsafe { nallocx(size, 0) };

		// SAFETY: every loop size is nonzero, and `QUANTUM` is a supported
		// power-of-two alignment.
		let aligned = unsafe { nallocx(size, MALLOCX_ALIGN(QUANTUM)) };

		assert_eq!(plain, aligned, "size {size} lands in a different class without the flag");
	}
}

/// Confirms that jemalloc's configured quantum covers the Rust-side constant.
///
/// `JEMALLOC_SYS_WITH_LG_QUANTUM` can lower it at configure time; this turns
/// an incompatible build into a deterministic test failure instead of silent
/// underalignment.
#[test]
fn jemalloc_quantum_covers_the_rust_quantum() {
	let quantum = ctl::quantum().unwrap();

	assert!(quantum >= QUANTUM, "jemalloc quantum {quantum} is below the assumed {QUANTUM}");
}

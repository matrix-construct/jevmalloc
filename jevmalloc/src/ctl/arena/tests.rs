//! Checks arena handle state and MIB substitution without mutating jemalloc.

use super::*;

/// Accepts the ordinary arena range and rejects reserved indices.
#[test]
fn validates_index_range() {
	// SAFETY: these handles are used only for numeric validation and never issue
	// a control operation, so no arena liveness assumption is required.
	let zero = unsafe { Arena::from_index(0) }.unwrap();

	// SAFETY: as above, this handle never reaches jemalloc.
	let last = unsafe { Arena::from_index(ARENA_INDEX_LIMIT - 1) }.unwrap();

	// SAFETY: range validation rejects the index before a handle is constructed.
	let error = unsafe { Arena::from_index(ARENA_INDEX_LIMIT) }.unwrap_err();

	assert_eq!(zero.index(), 0);
	assert_eq!(last.index(), ARENA_INDEX_LIMIT - 1);
	assert!(error.is(libc::EINVAL));
}

/// Substitutes one arena index at MIB component one.
#[test]
fn substitutes_arena_component() {
	// SAFETY: the fake handle is used only to inspect pure MIB substitution and
	// never reaches jemalloc.
	let arena = unsafe { Arena::from_index(7) }.unwrap();
	let key = arena.select(key::arena_decay().unwrap());

	assert_eq!(key[1], 7);
	assert!(!arena.is_owned());
}

#![cfg(test)]

//! Checks publication and reuse of a cached key.

extern crate std as rust_std;

use cache::Cache;
use rust_std::{sync::Barrier, thread};

use super::*;

/// Confirms that the published MIB matches a fresh translation.
#[test]
fn cached_key_matches_translation() {
	static CACHE: Cache = Cache::new();

	let first = CACHE.get("epoch").unwrap();
	let second = CACHE.get("epoch").unwrap();

	assert_eq!(first, raw::mibs("epoch").unwrap());
	assert_eq!(second, first);
}

/// Exercises concurrent first publication of one immutable key.
#[test]
fn publishes_concurrently() {
	static CACHE: Cache = Cache::new();
	static START: Barrier = Barrier::new(4);

	thread::scope(|scope| {
		for _ in 0..4 {
			scope.spawn(|| {
				START.wait();
				assert_eq!(CACHE.get("epoch").unwrap(), raw::mibs("epoch").unwrap());
			});
		}
	});
}

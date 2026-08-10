//! Exercises the typed `ctl` wrapper and its control-name requirements.
//!
//! The tests read and write `epoch` through `Access`, then confirm that empty
//! and non-null-terminated names are rejected.

#![cfg(test)]

use core::alloc::{GlobalAlloc, Layout};

use jevmalloc::{
	Jemalloc,
	ctl::{Access, AsName},
};

/// Installs jemalloc for the test process.
///
/// The smoke allocation exercises this global slot; the control wrappers
/// themselves delegate directly to jemalloc's raw control functions.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Checks a basic allocation while the control-test allocator is installed.
#[test]
fn smoke() {
	let layout = Layout::from_size_align(100, 8).unwrap();
	unsafe {
		let ptr = Jemalloc.alloc(layout);
		assert!(!ptr.is_null());
		Jemalloc.dealloc(ptr, layout);
	}
}

/// Checks typed reads and writes of the jemalloc `epoch` control.
#[test]
fn ctl_get_set() {
	let epoch: u64 = "epoch\0".name().read().unwrap();
	assert!(epoch > 0);
	"epoch\0".name().write(epoch).unwrap();
}

/// Confirms that reading through an empty control name is rejected.
///
/// # Panics
///
/// Always panics because [`AsName::name`] rejects an empty string.
#[test]
#[should_panic]
fn ctl_panic_empty_get() { let _: u64 = "".name().read().unwrap(); }

/// Confirms that writing through an empty control name is rejected.
///
/// # Panics
///
/// Always panics because [`AsName::name`] rejects an empty string.
#[test]
#[should_panic]
fn ctl_panic_empty_set() {
	let epoch: u64 = "epoch\0".name().read().unwrap();
	"".name().write(epoch).unwrap();
}

/// Confirms that reading through an unterminated control name is rejected.
///
/// # Panics
///
/// Always panics because [`AsName::name`] requires a null terminator.
#[test]
#[should_panic]
fn ctl_panic_non_null_terminated_get() { let _: u64 = "epoch".name().read().unwrap(); }

/// Confirms that writing through an unterminated control name is rejected.
///
/// # Panics
///
/// Always panics because [`AsName::name`] requires a null terminator.
#[test]
#[should_panic]
fn ctl_panic_non_null_terminated_set() {
	let epoch: u64 = "epoch\0".name().read().unwrap();
	"epoch".name().write(epoch).unwrap();
}

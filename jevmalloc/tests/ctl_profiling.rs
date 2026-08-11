//! Exercises profiling controls with runtime profiling initialized.

#![cfg(test)]

use jevmalloc::{Jemalloc, ctl};

/// Permits one static byte pointer to initialize the exported C string pointer.
union ConfigPtr {
	/// Pointer to the first configuration byte.
	byte: &'static u8,

	/// The same address with jemalloc's character type.
	char: &'static libc::c_char,
}

/// Configuration shared by both possible exported symbol names.
const CONFIG: Option<&'static libc::c_char> = Some(unsafe {
	ConfigPtr {
		byte: &b"prof:true,prof_active:false,prof_gdump:false\0"[0],
	}
	.char
});

/// Enables profiling for an unprefixed jemalloc build.
#[unsafe(export_name = "malloc_conf")]
pub static UNPREFIXED_MALLOC_CONF: Option<&'static libc::c_char> = CONFIG;

/// Enables profiling for a prefixed jemalloc build.
#[unsafe(export_name = "_rjem_malloc_conf")]
pub static PREFIXED_MALLOC_CONF: Option<&'static libc::c_char> = CONFIG;

/// Routes test-harness allocations through the configured jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Checks global and current-thread profiling state exchanges.
#[test]
fn profiling_state_round_trips() {
	let global = ctl::is_prof_enabled().unwrap();
	let thread = ctl::this_thread::is_prof_enabled().unwrap();
	let gdump_key = ctl::raw::mibs("prof.gdump").unwrap();
	let gdump = unsafe { ctl::raw::get::<bool>(&gdump_key) }.unwrap();

	assert_eq!(ctl::prof_enable(global).unwrap(), global);
	assert_eq!(ctl::prof_gdump(gdump).unwrap(), gdump);
	assert_eq!(ctl::this_thread::prof_enable(thread).unwrap(), thread);
	let _interval = ctl::prof_interval().unwrap();
}

/// Checks the profile reset command with its optional input omitted.
#[test]
fn profiling_reset_uses_the_current_sample_rate() { ctl::prof_reset().unwrap(); }

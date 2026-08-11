//! Exercises profiling controls with runtime profiling initialized.

#![cfg(test)]

use jevmalloc::{
	Jemalloc, ctl, is_prof_enabled, prof_enable, prof_gdump, prof_interval, prof_reset,
	this_thread,
};

/// Jemalloc's C `bool` representation when built by cl.exe.
#[cfg(target_env = "msvc")]
type CBool = libc::c_int;

/// Jemalloc's C `_Bool` representation on other targets.
#[cfg(not(target_env = "msvc"))]
type CBool = bool;

/// Permits one static byte pointer to initialize the exported C string pointer.
#[repr(C)]
union ConfigPtr {
	/// Pointer to the first configuration byte.
	byte: &'static u8,

	/// The same address with jemalloc's character type.
	char: &'static libc::c_char,
}

/// Configuration shared by both possible exported symbol names.
const CONFIG: Option<&'static libc::c_char> = Some(
	// SAFETY: `u8` and `c_char` have identical one-byte layouts, every bit
	// pattern is valid, and the referenced NUL-terminated bytes are static.
	unsafe {
		ConfigPtr {
			byte: &b"prof:true,prof_active:false,prof_gdump:false\0"[0],
		}
		.char
	},
);

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
	let global = is_prof_enabled().unwrap();
	let thread = this_thread::is_prof_enabled().unwrap();
	let gdump_key = ctl::raw::mibs("prof.gdump").unwrap();

	// SAFETY: this is the complete `prof.gdump` MIB, and `CBool` matches the
	// platform C `bool` representation.
	let gdump = unsafe { ctl::raw::get::<CBool>(&gdump_key) }.unwrap();
	let gdump = gdump != CBool::default();

	assert_eq!(prof_enable(global).unwrap(), global);
	assert_eq!(prof_gdump(gdump).unwrap(), gdump);
	assert_eq!(this_thread::prof_enable(thread).unwrap(), thread);
	let _interval = prof_interval().unwrap();
}

/// Checks the profile reset command with its optional input omitted.
#[test]
fn profiling_reset_uses_the_current_sample_rate() { prof_reset().unwrap(); }

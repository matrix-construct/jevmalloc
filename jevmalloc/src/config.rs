//! Jemalloc build-time configuration.
//!
//! These getters expose the documented `config.*` controls. Boolean values
//! report whether a capability was selected when jemalloc was built. The
//! embedded allocation configuration is returned as a borrowed C string and
//! requires no allocation or encoding conversion. A library supplied through
//! `JEMALLOC_OVERRIDE` must preserve the bundled implementation's immutable,
//! process-lifetime storage contract for string controls.

use core::ffi::CStr;

use crate::ctl::{Result, key, value};

/// Defines a getter for one boolean build configuration.
macro_rules! bool_getter {
	($(#[$meta:meta])* $name:ident => $key:ident) => {
		$(#[$meta])*
		///
		/// # Errors
		///
		/// Returns an error if jemalloc rejects the query.
		pub fn $name() -> Result<bool> {
			let key = key::$key()?;
			value::get_bool(&key)
		}
	};
}

bool_getter! {
	/// Returns whether cache-oblivious large-allocation alignment was enabled.
	cache_oblivious => config_cache_oblivious
}

bool_getter! {
	/// Returns whether jemalloc was built with debug assertions and safeguards.
	debug => config_debug
}

bool_getter! {
	/// Returns whether allocation filling options were enabled.
	fill => config_fill
}

bool_getter! {
	/// Returns whether lazy locking was enabled.
	lazy_lock => config_lazy_lock
}

/// Returns the embedded configure-time allocation options.
///
/// The string is empty unless jemalloc was built with an embedded runtime
/// configuration. Its bytes remain valid for the life of the process.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query or returns a null pointer.
pub fn malloc_conf() -> Result<&'static CStr> {
	let key = key::config_malloc_conf()?;
	value::get_cstr(&key)
}

bool_getter! {
	/// Returns whether heap profiling support was enabled.
	prof => config_prof
}

bool_getter! {
	/// Returns whether profiling backtraces use the libgcc unwinder.
	prof_libgcc => config_prof_libgcc
}

bool_getter! {
	/// Returns whether profiling backtraces use libunwind.
	prof_libunwind => config_prof_libunwind
}

bool_getter! {
	/// Returns whether allocator statistics were enabled.
	stats => config_stats
}

bool_getter! {
	/// Returns whether utrace support was enabled.
	utrace => config_utrace
}

bool_getter! {
	/// Returns whether abort-on-allocation-failure support was enabled.
	xmalloc => config_xmalloc
}

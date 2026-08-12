//! Jemalloc startup options.
//!
//! These getters expose the documented `opt.*` controls. Jemalloc resolves the
//! options during initialization, so this interface is read-only. Getters that
//! depend on fill, utrace, xmalloc, or profiling support return `ENOENT` when
//! that capability was omitted from the jemalloc build. String values borrow
//! process-lifetime storage and require no allocation or encoding conversion.
//! A library supplied through `JEMALLOC_OVERRIDE` must preserve the bundled
//! implementation's immutable, process-lifetime storage contract for them.

use core::ffi::CStr;

use libc::c_uint;

use crate::ctl::{Result, key, raw, value};

/// Defines a getter for one boolean startup option.
macro_rules! bool_getter {
	($(#[$meta:meta])* $name:ident => $key:ident) => {
		$(#[$meta])*
		///
		/// # Errors
		///
		/// Returns an error if jemalloc rejects or does not provide the query.
		pub fn $name() -> Result<bool> {
			let key = key::$key()?;
			value::get_bool(&key)
		}
	};
}

/// Defines a getter for one scalar startup option.
macro_rules! scalar_getter {
	($(#[$meta:meta])* $name:ident => $key:ident: $value:ty) => {
		$(#[$meta])*
		///
		/// # Errors
		///
		/// Returns an error if jemalloc rejects or does not provide the query.
		pub fn $name() -> Result<$value> {
			let key = key::$key()?;

			// SAFETY: this invocation pairs the control with its documented C
			// output type.
			unsafe { raw::get::<$value>(&key) }
		}
	};
}

/// Defines a getter for one process-lifetime string startup option.
macro_rules! cstr_getter {
	($(#[$meta:meta])* $name:ident => $key:ident) => {
		$(#[$meta])*
		///
		/// The returned bytes remain valid for the life of the process.
		///
		/// # Errors
		///
		/// Returns an error if jemalloc rejects the query or returns a null
		/// pointer.
		pub fn $name() -> Result<&'static CStr> {
			let key = key::$key()?;
			value::get_cstr(&key)
		}
	};
}

bool_getter! {
	/// Returns whether most jemalloc warnings abort the process.
	abort => opt_abort
}

bool_getter! {
	/// Returns whether resolved startup options are printed during initialization.
	confirm_conf => opt_confirm_conf
}

bool_getter! {
	/// Returns whether invalid runtime configuration aborts the process.
	abort_conf => opt_abort_conf
}

bool_getter! {
	/// Returns whether large allocations use cache-oblivious alignment.
	cache_oblivious => opt_cache_oblivious
}

cstr_getter! {
	/// Returns the transparent-huge-page mode for allocator metadata.
	metadata_thp => opt_metadata_thp
}

bool_getter! {
	/// Returns whether jemalloc trusts `MADV_DONTNEED` to zero pages.
	trust_madvise => opt_trust_madvise
}

bool_getter! {
	/// Returns whether unused virtual memory is retained for later reuse.
	retain => opt_retain
}

cstr_getter! {
	/// Returns the dynamic storage allocation precedence.
	dss => opt_dss
}

scalar_getter! {
	/// Returns the maximum number of automatically managed arenas.
	narenas => opt_narenas: c_uint
}

scalar_getter! {
	/// Returns the byte threshold above which requests use the oversize arena.
	oversize_threshold => opt_oversize_threshold: usize
}

cstr_getter! {
	/// Returns the per-CPU arena mode.
	percpu_arena => opt_percpu_arena
}

bool_getter! {
	/// Returns whether background worker threads are enabled at initialization.
	background_thread => opt_background_thread
}

scalar_getter! {
	/// Returns the configured maximum number of background worker threads.
	max_background_threads => opt_max_background_threads: usize
}

scalar_getter! {
	/// Returns the dirty-page decay interval in milliseconds.
	dirty_decay_ms => opt_dirty_decay_ms: isize
}

scalar_getter! {
	/// Returns the muzzy-page decay interval in milliseconds.
	muzzy_decay_ms => opt_muzzy_decay_ms: isize
}

scalar_getter! {
	/// Returns the base-two logarithm of the maximum active-extent fit ratio.
	lg_extent_max_active_fit => opt_lg_extent_max_active_fit: usize
}

bool_getter! {
	/// Returns whether allocator statistics are printed at process exit.
	stats_print => opt_stats_print
}

cstr_getter! {
	/// Returns the output options used for exit-time statistics.
	stats_print_opts => opt_stats_print_opts
}

scalar_getter! {
	/// Returns the average allocation-activity byte interval between statistics outputs.
	stats_interval => opt_stats_interval: i64
}

cstr_getter! {
	/// Returns the output options used for interval statistics.
	stats_interval_opts => opt_stats_interval_opts
}

cstr_getter! {
	/// Returns the allocation junk-filling mode.
	junk => opt_junk
}

bool_getter! {
	/// Returns whether newly allocated memory is zero-filled by default.
	zero => opt_zero
}

bool_getter! {
	/// Returns whether allocation events are traced with utrace.
	utrace => opt_utrace
}

bool_getter! {
	/// Returns whether allocation failure aborts the process.
	xmalloc => opt_xmalloc
}

bool_getter! {
	/// Returns whether thread-specific allocation caching is enabled by default.
	tcache => opt_tcache
}

scalar_getter! {
	/// Returns the largest size class eligible for thread caching, in bytes.
	tcache_max => opt_tcache_max: usize
}

cstr_getter! {
	/// Returns the transparent-huge-page mode for user allocation mappings.
	thp => opt_thp
}

scalar_getter! {
	/// Returns the maximum number of frames captured in a profile backtrace.
	prof_bt_max => opt_prof_bt_max: c_uint
}

bool_getter! {
	/// Returns whether heap profiling is enabled at initialization.
	prof => opt_prof
}

cstr_getter! {
	/// Returns the filename prefix used for automatic heap profile dumps.
	prof_prefix => opt_prof_prefix
}

bool_getter! {
	/// Returns whether profiling is globally active at initialization.
	prof_active => opt_prof_active
}

bool_getter! {
	/// Returns the initial per-thread profiling-active setting.
	prof_thread_active_init => opt_prof_thread_active_init
}

scalar_getter! {
	/// Returns the base-two logarithm of the mean profile sample interval.
	lg_prof_sample => opt_lg_prof_sample: usize
}

bool_getter! {
	/// Returns whether profiling accumulates sampled object counts over time.
	prof_accum => opt_prof_accum
}

bool_getter! {
	/// Returns whether profile filenames include a PID namespace identifier.
	prof_pid_namespace => opt_prof_pid_namespace
}

scalar_getter! {
	/// Returns the base-two logarithm of the profile dump byte interval.
	lg_prof_interval => opt_lg_prof_interval: isize
}

bool_getter! {
	/// Returns whether profiles dump at virtual-memory high-water marks.
	prof_gdump => opt_prof_gdump
}

bool_getter! {
	/// Returns whether a final profile is dumped at process exit.
	prof_final => opt_prof_final
}

bool_getter! {
	/// Returns whether final profiling output performs leak checking.
	prof_leak => opt_prof_leak
}

bool_getter! {
	/// Returns whether detected profile leaks set a failure exit code.
	prof_leak_error => opt_prof_leak_error
}

cstr_getter! {
	/// Returns the configured behavior for reallocating a non-null pointer to zero.
	zero_realloc => opt_zero_realloc
}

scalar_getter! {
	/// Returns the maximum tcache entries scanned for debug double-free detection.
	debug_double_free_max_scan => opt_debug_double_free_max_scan: c_uint
}

bool_getter! {
	/// Returns whether the standard large size-class hierarchy is disabled.
	disable_large_size_classes => opt_disable_large_size_classes
}

scalar_getter! {
	/// Returns the maximum number of extents in one `process_madvise` batch.
	process_madvise_max_batch => opt_process_madvise_max_batch: usize
}

//! Exercises the documented startup-option getters.

#![cfg(test)]

use jevmalloc::{Jemalloc, config, ctl, opt};

/// Routes the test harness through the observed jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Requires one unconditional option getter to succeed.
fn succeeds<T>(result: ctl::Result<T>) {
	let _value = result.unwrap_or_else(|error| panic!("option failed: {error}"));
}

/// Checks the documented conditional-option behavior.
fn matches_availability<T>(available: bool, result: ctl::Result<T>) {
	match (available, result) {
		| (true, Ok(_)) => {},
		| (false, Err(error)) if error.is(libc::ENOENT) => {},
		| (true, Err(error)) => panic!("available option failed: {error}"),
		| (false, Ok(_)) => panic!("unavailable option unexpectedly succeeded"),
		| (false, Err(error)) => panic!("unavailable option returned {error}"),
	}
}

/// Checks every documented `opt.*` control and its build requirements.
#[test]
fn documented_opt_getters_match_the_build() {
	succeeds(opt::abort());
	succeeds(opt::confirm_conf());
	succeeds(opt::abort_conf());
	succeeds(opt::cache_oblivious());
	succeeds(opt::metadata_thp());
	succeeds(opt::trust_madvise());
	succeeds(opt::retain());
	succeeds(opt::dss());
	succeeds(opt::narenas());
	succeeds(opt::oversize_threshold());
	succeeds(opt::percpu_arena());
	succeeds(opt::background_thread());
	succeeds(opt::max_background_threads());
	succeeds(opt::dirty_decay_ms());
	succeeds(opt::muzzy_decay_ms());
	succeeds(opt::lg_extent_max_active_fit());
	succeeds(opt::stats_print());
	succeeds(opt::stats_print_opts());
	succeeds(opt::stats_interval());
	succeeds(opt::stats_interval_opts());

	let fill = config::fill().unwrap();
	matches_availability(fill, opt::junk());
	matches_availability(fill, opt::zero());

	let utrace = config::utrace().unwrap();
	matches_availability(utrace, opt::utrace());

	let xmalloc = config::xmalloc().unwrap();
	matches_availability(xmalloc, opt::xmalloc());

	succeeds(opt::tcache());
	succeeds(opt::tcache_max());
	succeeds(opt::thp());

	let prof = config::prof().unwrap();
	matches_availability(prof, opt::prof_bt_max());
	matches_availability(prof, opt::prof());
	matches_availability(prof, opt::prof_prefix());
	matches_availability(prof, opt::prof_active());
	matches_availability(prof, opt::prof_thread_active_init());
	matches_availability(prof, opt::lg_prof_sample());
	matches_availability(prof, opt::prof_accum());
	matches_availability(prof, opt::prof_pid_namespace());
	matches_availability(prof, opt::lg_prof_interval());
	matches_availability(prof, opt::prof_gdump());
	matches_availability(prof, opt::prof_final());
	matches_availability(prof, opt::prof_leak());
	matches_availability(prof, opt::prof_leak_error());

	succeeds(opt::zero_realloc());
	succeeds(opt::debug_double_free_max_scan());
	succeeds(opt::disable_large_size_classes());
	succeeds(opt::process_madvise_max_batch());
}

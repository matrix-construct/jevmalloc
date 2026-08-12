//! Process-wide caching for built-in MIB keys.

/// Caches one fixed control name as an inline MIB.
mod cache;

#[cfg(test)]
mod tests;

use self::cache::Cache;
use super::{Error, Result, raw};

/// A resolved jemalloc Management Information Base key.
///
/// Jemalloc currently uses at most seven numeric components. The extra slot
/// leaves room for compatible namespace growth while keeping every key inline.
pub type Key = arrayvec::ArrayVec<usize, KEY_SEGS>;

/// Maximum number of bytes in a control name, including its trailing NUL.
pub const NAME_MAX: usize = 128;

/// Number of inline numeric components reserved for a MIB key.
pub const KEY_SEGS: usize = 8;

/// Defines one process-wide cache and accessor for a fixed control name.
macro_rules! define_key {
	($accessor:ident, $name:literal) => {
		#[doc = concat!("Returns the cached MIB for `", $name, "`.")]
		pub(crate) fn $accessor() -> Result<Key> {
			static KEY: Cache = Cache::new();
			KEY.get($name)
		}
	};
}

define_key!(epoch, "epoch");
define_key!(background_thread, "background_thread");

define_key!(config_cache_oblivious, "config.cache_oblivious");
define_key!(config_debug, "config.debug");
define_key!(config_fill, "config.fill");
define_key!(config_lazy_lock, "config.lazy_lock");
define_key!(config_malloc_conf, "config.malloc_conf");
define_key!(config_prof, "config.prof");
define_key!(config_prof_libgcc, "config.prof_libgcc");
define_key!(config_prof_libunwind, "config.prof_libunwind");
define_key!(config_stats, "config.stats");
define_key!(config_utrace, "config.utrace");
define_key!(config_xmalloc, "config.xmalloc");

define_key!(opt_abort, "opt.abort");
define_key!(opt_confirm_conf, "opt.confirm_conf");
define_key!(opt_abort_conf, "opt.abort_conf");
define_key!(opt_cache_oblivious, "opt.cache_oblivious");
define_key!(opt_metadata_thp, "opt.metadata_thp");
define_key!(opt_trust_madvise, "opt.trust_madvise");
define_key!(opt_retain, "opt.retain");
define_key!(opt_dss, "opt.dss");
define_key!(opt_narenas, "opt.narenas");
define_key!(opt_oversize_threshold, "opt.oversize_threshold");
define_key!(opt_percpu_arena, "opt.percpu_arena");
define_key!(opt_background_thread, "opt.background_thread");
define_key!(opt_max_background_threads, "opt.max_background_threads");
define_key!(opt_dirty_decay_ms, "opt.dirty_decay_ms");
define_key!(opt_muzzy_decay_ms, "opt.muzzy_decay_ms");
define_key!(opt_lg_extent_max_active_fit, "opt.lg_extent_max_active_fit");
define_key!(opt_stats_print, "opt.stats_print");
define_key!(opt_stats_print_opts, "opt.stats_print_opts");
define_key!(opt_stats_interval, "opt.stats_interval");
define_key!(opt_stats_interval_opts, "opt.stats_interval_opts");
define_key!(opt_junk, "opt.junk");
define_key!(opt_zero, "opt.zero");
define_key!(opt_utrace, "opt.utrace");
define_key!(opt_xmalloc, "opt.xmalloc");
define_key!(opt_tcache, "opt.tcache");
define_key!(opt_tcache_max, "opt.tcache_max");
define_key!(opt_thp, "opt.thp");
define_key!(opt_prof_bt_max, "opt.prof_bt_max");
define_key!(opt_prof, "opt.prof");
define_key!(opt_prof_prefix, "opt.prof_prefix");
define_key!(opt_prof_active, "opt.prof_active");
define_key!(opt_prof_thread_active_init, "opt.prof_thread_active_init");
define_key!(opt_lg_prof_sample, "opt.lg_prof_sample");
define_key!(opt_prof_accum, "opt.prof_accum");
define_key!(opt_prof_pid_namespace, "opt.prof_pid_namespace");
define_key!(opt_lg_prof_interval, "opt.lg_prof_interval");
define_key!(opt_prof_gdump, "opt.prof_gdump");
define_key!(opt_prof_final, "opt.prof_final");
define_key!(opt_prof_leak, "opt.prof_leak");
define_key!(opt_prof_leak_error, "opt.prof_leak_error");
define_key!(opt_zero_realloc, "opt.zero_realloc");
define_key!(opt_debug_double_free_max_scan, "opt.debug_double_free_max_scan");
define_key!(opt_disable_large_size_classes, "opt.disable_large_size_classes");
define_key!(opt_process_madvise_max_batch, "opt.process_madvise_max_batch");

define_key!(arena_purge, "arena.0.purge");
define_key!(arena_decay, "arena.0.decay");
define_key!(arena_reset, "arena.0.reset");
define_key!(arena_destroy, "arena.0.destroy");
define_key!(arena_name, "arena.0.name");
define_key!(arena_dss, "arena.0.dss");
define_key!(arena_muzzy_decay, "arena.0.muzzy_decay_ms");
define_key!(arena_dirty_decay, "arena.0.dirty_decay_ms");
define_key!(arena_retain_grow_limit, "arena.0.retain_grow_limit");
define_key!(arena_extent_hooks, "arena.0.extent_hooks");
define_key!(arenas_create, "arenas.create");
define_key!(arenas_lookup, "arenas.lookup");
define_key!(arenas_muzzy_decay, "arenas.muzzy_decay_ms");
define_key!(arenas_dirty_decay, "arenas.dirty_decay_ms");
define_key!(arenas_limit, "arenas.narenas");
define_key!(arenas_quantum, "arenas.quantum");
define_key!(thread_idle, "thread.idle");
define_key!(thread_arena, "thread.arena");
define_key!(thread_tcache_flush, "thread.tcache.flush");
define_key!(thread_tcache_enabled, "thread.tcache.enabled");
define_key!(thread_tcache_max, "thread.tcache.max");
define_key!(
	thread_tcache_ncached_max_read_sizeclass,
	"thread.tcache.ncached_max.read_sizeclass"
);
define_key!(thread_tcache_ncached_max_write, "thread.tcache.ncached_max.write");
define_key!(tcache_create, "tcache.create");
define_key!(tcache_flush, "tcache.flush");
define_key!(tcache_destroy, "tcache.destroy");

#[cfg(feature = "profiling")]
define_key!(prof_reset, "prof.reset");
#[cfg(feature = "profiling")]
define_key!(prof_dump, "prof.dump");
#[cfg(feature = "profiling")]
define_key!(prof_gdump, "prof.gdump");
#[cfg(feature = "profiling")]
define_key!(prof_active, "prof.active");
#[cfg(feature = "profiling")]
define_key!(prof_interval, "prof.interval");
#[cfg(feature = "profiling")]
define_key!(thread_prof_active, "thread.prof.active");

#[cfg(feature = "stats")]
define_key!(stats_mutexes_reset, "stats.mutexes.reset");
#[cfg(feature = "stats")]
define_key!(stats_allocated, "stats.allocated");
#[cfg(feature = "stats")]
define_key!(stats_active, "stats.active");
#[cfg(feature = "stats")]
define_key!(stats_metadata, "stats.metadata");
#[cfg(feature = "stats")]
define_key!(stats_metadata_thp, "stats.metadata_thp");
#[cfg(feature = "stats")]
define_key!(stats_resident, "stats.resident");
#[cfg(feature = "stats")]
define_key!(stats_mapped, "stats.mapped");
#[cfg(feature = "stats")]
define_key!(stats_retained, "stats.retained");
#[cfg(feature = "stats")]
define_key!(stats_zero_reallocs, "stats.zero_reallocs");
#[cfg(feature = "stats")]
define_key!(stats_background_thread_num_threads, "stats.background_thread.num_threads");
#[cfg(feature = "stats")]
define_key!(stats_background_thread_num_runs, "stats.background_thread.num_runs");
#[cfg(feature = "stats")]
define_key!(stats_background_thread_run_interval, "stats.background_thread.run_interval");
#[cfg(feature = "stats")]
define_key!(thread_peak_reset, "thread.peak.reset");
#[cfg(feature = "stats")]
define_key!(thread_peak_read, "thread.peak.read");
#[cfg(feature = "stats")]
define_key!(thread_allocated, "thread.allocated");
#[cfg(feature = "stats")]
define_key!(thread_deallocated, "thread.deallocated");
#[cfg(feature = "stats")]
define_key!(thread_allocatedp, "thread.allocatedp");
#[cfg(feature = "stats")]
define_key!(thread_deallocatedp, "thread.deallocatedp");

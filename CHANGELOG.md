# Unreleased

- Model explicitly created jemalloc arenas as lifecycle-owning `jevmalloc::Arena`
  objects. Cover every documented non-statistics instance control, custom extent
  hook construction and replacement, allocation lookup, recoverable destruction,
  and allocator-wide defaults under `jevmalloc::arenas`.
- Replace the generated TikV-style `ctl` option tree with a compact MIB-only
  control interface. Built-in MIBs use process-wide caches.
- Organize typed allocator operations at crate root and under `arena`, `arenas`,
  `profiling`, `stats`, and `this_thread`, leaving MIB access under `ctl`.
- Make generic raw MIB operations explicitly unsafe, preserve numeric errno
  values, validate ad hoc names, require exact value sizes, and expose command
  preconditions at the safety boundary.
- Make statistics refresh explicit through `stats::refresh_epoch`; ordinary
  control reads no longer refresh every arena. Add
  `this_thread::ThreadCounters`, a thread-confined direct counter handle, instead
  of exposing allocator-mutated counters through static shared references.
- Update vendored `jemalloc` to 5.3.1, a 396-commit catch-up over 5.3.0. The
  checked-in `configure` was regenerated from the new `configure.ac`; the option
  set is purely additive, so no build glue changed.
- Add the ISO C23 sized deallocations `free_sized` and `free_aligned_sized` to
  `jevmalloc-sys`. Neither accepts a null pointer, contrary to C23: both forward
  to `sdallocx`, which never treats null as a no-op.
- Add the `pageid` feature (`--enable-pageid`), which names jemalloc's mappings
  in `/proc/<pid>/maps`, and `profiling_frameptr` (`--enable-prof-frameptr`).
- `jevmalloc::ctl::Error` now implements `core::error::Error` unconditionally;
  it was previously behind a `use_std` feature that did not exist.
- Add `single_allocator` tests asserting which allocator services the process in
  each symbol regime, and port the roundtrip benchmarks to `GlobalAlloc`.
- `GlobalAlloc` now omits `MALLOCX_ALIGN` when the size class already satisfies
  the alignment, through the new `layout_flags`. jemalloc tests the flag word
  before it inspects the pointer, so passing the alignment unconditionally had
  kept every ordinary Rust allocation off the thread-cache fast path.

Everything below this line predates the fork and refers to the `tikv-jemalloc*`
crates this workspace was derived from.

# 0.6.0 - 2024-07-14

- Fix build on riscv64gc-unknown-linux-musl (#67) (#75)
- Allow jemalloc-sys to be the default allocator on musl linux (#70)
- Add Chimera Linux to gmake targets (#73)
- Add profiling options to jemalloc-ctl (#74)
- Fix jemalloc version not shown in API (#77)
- Fix jemalloc stats is still enabled when stats feature is disabled (#82)
- Fix duplicated symbol when build and link on aarch64-linux-android (#83)
- Revise CI runner platform on macOS (#86)
- Allow setting per-target env (#91)
- Remove outdated clippy allows (#94)
- Set MSRV to 1.71.0 (#95)

Note since 0.6.0, if you want to use jemalloc stats, you have to enable the
feature explicitly.

# 0.5.4 - 2023-07-22

- Add disable_initial_exec_tls feature for jemalloc-ctl (#59)
- Fix definition of `c_bool` for non-MSVC targets (#54)
- Add `disable_cache_oblivious` feature (#51)
- Add loongarch64 support (#42)

# jemalloc-sys 0.5.3 - 2023-02-03

- Remove fs-extra dependency (#47)

# jemalloc-sys 0.5.2 - 2022-09-29

- Fix build on riscv64gc-unknown-linux-gnu (#40)

# jemalloc-sys 0.5.1 - 2022-06-22

- Backport support for NetBSD (#31)
- Watch environment variable change in build script (#31)

# 0.5.0 - 2022-05-19

- Update jemalloc to 5.3.0 (#23)

# 0.4.3 - 2022-02-21

- Added riscv64 support (#14)

# 0.4.2 - 2021-08-09

- Fixed prof not working under certain condition (#9) (#12)
- Updated paste to 1 (#11)

# 0.4.1 - 2020-11-16

- Updated jemalloc to fix deadlock during initialization
- Fixed failure of generating docs on release version

# 0.4.0 - 2020-07-21

- Forked from jemallocator master
- Upgraded jemalloc to 5.2.1 (#1)
- Fixed wrong version in generated C header (#1)
- Upgraded project to 2018 edition (#2)

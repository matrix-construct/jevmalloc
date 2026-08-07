# jevmalloc-sys - Rust bindings to the `jemalloc` C library

[![ci]][github actions]

> Note: the Rust allocator API is implemented for `jemalloc` in the sibling
> [`jevmalloc`](../jevmalloc) crate, which also re-exports these bindings as
> `jevmalloc::ffi`.

`jemalloc` is a general purpose memory allocator; its documentation can be found
here:

* [API documentation][jemalloc_docs]
* [Wiki][jemalloc_wiki] (design documents, presentations, profiling, debugging, tuning, ...)

[jemalloc_docs]: http://jemalloc.net/jemalloc.3.html
[jemalloc_wiki]: https://github.com/jemalloc/jemalloc/wiki

**Current jemalloc version**: 5.3.1.

The C source is vendored as the `jemalloc` submodule, tracking
[`matrix-construct/jemalloc`](https://github.com/matrix-construct/jemalloc).
Note that `build.rs` configures the build with the **checked-in
`configure/configure`**, not the one the submodule would generate; see
[`update_jemalloc.md`](update_jemalloc.md) before moving the submodule.

## Platform support

See the platform support table in the [workspace README](../README.md#platform-support).

## Features

Each feature corresponds to a `jemalloc` `configure` option; the reference is
[`jemalloc/INSTALL.md`][jemalloc_install]. `build.rs` passes an explicit
`--enable-`/`--disable-` pair for each one (the Linux-gated `profiling_frameptr`
is the enable-only exception), so a feature being off is a positive instruction,
not a default.

Default: `cache_oblivious`, `initial_exec_tls`,
`unprefixed_malloc_on_supported_platforms`.

* `unprefixed_malloc_on_supported_platforms` (default): when **disabled**,
  configures `jemalloc` with `--with-jemalloc-prefix=_rjem_`. Enabling it emits
  symbols like `malloc` without a prefix, overriding the ones defined by libc.
  This usually causes C and C++ code linked into the same program to use
  `jemalloc` as well. On the targets in `NO_UNPREFIXED_MALLOC_TARGETS` the
  prefix is applied regardless, because unprefixing is known to segfault there
  from allocator mismatches.

* `cache_oblivious` (default, `--enable-cache-oblivious`): when disabled, all
  large allocations are page-aligned as an implementation artifact, which can
  severely harm CPU cache utilization. The cache-oblivious layout costs one
  extra page per large allocation, which can be infeasible for some
  applications.

* `initial_exec_tls` (default, `--enable-initial-exec-tls`): uses the
  initial-exec TLS model for `jemalloc`'s internal thread-local storage.
  Disable it to allow `jemalloc` to be loaded after program startup via
  `dlopen`; the symptom is `yourlib.so: cannot allocate memory in static TLS
  block`.

* `stats` (`--enable-stats`): enables statistics gathering. See `jemalloc`'s
  `opt.stats_print` documentation, and note that the `stats.*` MALLCTL subtree
  is absent without this.

* `profiling` (`--enable-prof`): enables heap profiling and leak detection. See
  `opt.prof`. There are several approaches to backtracing, and the configure
  script picks the first that works:

  * `libunwind` (requires `--enable-prof-libunwind`)
  * frame pointer (see `profiling_frameptr` below)
  * `libgcc` (unless `--disable-prof-libgcc`)
  * `gcc intrinsics` (unless `--disable-prof-gcc`)

* `profiling_frameptr` (implies `profiling`, `--enable-prof-frameptr`): uses the
  optimized frame-pointer unwinder, and adds `-fno-omit-frame-pointer` to the
  `jemalloc` build. `jemalloc` registers this option on Linux only, so the flag
  is not passed on other targets. It takes precedence over `libgcc` and the gcc
  intrinsics, but not over `libunwind`.

* `pageid` (`--enable-pageid`): names `jemalloc`'s mappings via
  `prctl(PR_SET_VMA_ANON_NAME)`, so they appear in `/proc/<pid>/maps` as
  `[anon:jemalloc_pg]` / `[anon:jemalloc_pg_overcommit]`. Linux only; costs one
  `prctl` per mapping and makes the allocator's share of the address space
  directly observable.

* `fill` (`--enable-fill`): enables junk/zero filling of allocated and
  deallocated memory, controlled at run time by `opt.junk` and `opt.zero`.

* `check_safety` (`--enable-opt-safety-checks`): enables the `opt.safety_checks`
  run-time consistency checks.

* `check_size_match` (`--enable-opt-size-checks`): validates the size passed to
  a sized deallocation against the true allocation size, aborting on a mismatch.

* `check_use_after_free` (implies `fill`, `--enable-uaf-detection`): enables
  use-after-free detection.

* `paranoid`: shorthand for `check_safety` + `check_size_match` +
  `check_use_after_free`. Substantial performance cost; for development.

`--enable-debug` is not a feature: `build.rs` derives it from
`debug_assertions`, so a debug Cargo profile builds a debug `jemalloc`.

## Running `jemalloc`'s own test suite

Set `JEMALLOC_SYS_RUN_JEMALLOC_TESTS=1` and build the crate. This runs `make
check` in the vendored source as part of the build script, and fails the build
if any of its ~1800 test cases fail. It is the real signal after a source bump.
The variable is deliberately not watched for changes, so switch build
directories (`CARGO_TARGET_DIR`) rather than expecting a rebuild.

## Environment variables

`jemalloc` options taking values are passed via environment variables using the
schema `JEMALLOC_SYS_{KEY}=VALUE` where the `KEY` names correspond to the
`./configure` options of `jemalloc` where the words are capitalized and the
hyphens `-` are replaced with underscores `_`(see
[`jemalloc/INSTALL.md`][jemalloc_install]). Each is also read under a
target-prefixed name, e.g. `X86_64_UNKNOWN_LINUX_GNU_JEMALLOC_SYS_WITH_LG_PAGE`.

* `JEMALLOC_OVERRIDE=<path/to/libjemalloc.a>`: skip building the vendored source
  entirely and link the named library instead. `build.rs` returns before it ever
  runs `configure`, so **no feature on this crate affects that build**.

* `JEMALLOC_SYS_WITH_MALLOC_CONF=<malloc_conf>`: Embed `<malloc_conf>` as a
  run-time options string that is processed prior to the `malloc_conf` global
  variable, the `/etc/malloc.conf` symlink, and the `MALLOC_CONF` environment
  variable (note: this variable might be prefixed as `_RJEM_MALLOC_CONF`). For
  example, to change the default decay time for dirty pages to 30 seconds:

  ```
  JEMALLOC_SYS_WITH_MALLOC_CONF=dirty_decay_ms:30000
  ```

* `JEMALLOC_SYS_WITH_LG_PAGE=<lg-page>`: Specify the base 2 log of the allocator
  page size, which must in turn be at least as large as the system page size. By
  default the configure script determines the host's page size and sets the
  allocator page size equal to the system page size, so this option need not be
  specified unless the system page size may change between configuration and
  execution, e.g. when cross compiling. Note that jemalloc 5.3.1 changed the
  default on aarch64 Linux to 64 KiB.

* `JEMALLOC_SYS_WITH_LG_HUGEPAGE=<lg-hugepage>`: Specify the base 2 log of the
  system huge page size. This option is useful when cross compiling, or when
  overriding the default for systems that do not explicitly support huge pages.

* `JEMALLOC_SYS_WITH_LG_QUANTUM=<lg-quantum>`: Specify the base 2 log of the
  minimum allocation alignment. jemalloc needs to know the minimum alignment
  that meets the following C standard requirement (quoted from the April 12,
  2011 draft of the C11 standard):

  > The pointer returned if the allocation succeeds is suitably aligned so that
  > it may be assigned to a pointer to any type of object with a fundamental
  > alignment requirement and then used to access such an object or an array of
  > such objects in the space allocated [...]

  This setting is architecture-specific, and although jemalloc includes known
  safe values for the most commonly used modern architectures, there is a
  wrinkle related to GNU libc (glibc) that may impact your choice of . On most
  modern architectures, this mandates 16-byte alignment (=4), but the glibc
  developers chose not to meet this requirement for performance reasons. An old
  discussion can be found at https://sourceware.org/bugzilla/show_bug.cgi?id=206
  . Unlike glibc, jemalloc does follow the C standard by default (caveat:
  jemalloc technically cheats for size classes smaller than the quantum), but
  the fact that Linux systems already work around this allocator noncompliance
  means that it is generally safe in practice to let jemalloc's minimum
  alignment follow glibc's lead. If you specify `JEMALLOC_SYS_WITH_LG_QUANTUM=3`
  during configuration, jemalloc will provide additional size classes that are
  not 16-byte-aligned (24, 40, and 56).

* `JEMALLOC_SYS_WITH_LG_VADDR=<lg-vaddr>`: Specify the number of significant
  virtual address bits. By default, the configure script attempts to detect
  virtual address size on those platforms where it knows how, and picks a
  default otherwise. This option may be useful when cross-compiling.

[jemalloc_install]: https://github.com/jemalloc/jemalloc/blob/dev/INSTALL.md#advanced-configuration

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `jevmalloc-sys` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[ci]: https://github.com/matrix-construct/jevmalloc/actions/workflows/main.yml/badge.svg
[github actions]: https://github.com/matrix-construct/jevmalloc/actions

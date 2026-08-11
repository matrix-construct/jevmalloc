# jevmalloc

[![ci]][github actions]

> Links against `jemalloc` and provides a `Jemalloc` unit type that implements
> `GlobalAlloc` and can be set as the `#[global_allocator]`

A hard fork of [jemallocator](https://github.com/tikv/jemallocator), itself the
successor of [gnzlbg/jemallocator](https://github.com/gnzlbg/jemallocator). Most
of the upstream surface has been refactored or removed, and the crate is not
published to crates.io; depend on it by git.

## Overview

Two crates, and the C source they vendor:

* `jevmalloc-sys`: builds and links `jemalloc`, exposing raw C bindings to it.
  The C source is the `jevmalloc-sys/jemalloc` submodule
  ([`matrix-construct/jemalloc`](https://github.com/matrix-construct/jemalloc));
  see [`jevmalloc-sys/update_jemalloc.md`](jevmalloc-sys/update_jemalloc.md) for
  how to move it.
* `jevmalloc`: provides the `Jemalloc` type implementing `GlobalAlloc`, a
  re-export of the raw bindings as `jevmalloc::ffi`, typed allocator operations
  grouped by scope, and low-level MIB access under `jevmalloc::ctl`.

## Usage

```toml
# Cargo.toml
[target.'cfg(not(target_env = "msvc"))'.dependencies]
jevmalloc = { git = "https://github.com/matrix-construct/jevmalloc" }
```

To set `jevmalloc::Jemalloc` as the global allocator:

```rust
// main.rs
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: jevmalloc::Jemalloc = jevmalloc::Jemalloc;
```

And that's it! Once you've defined this `static` then jemalloc will be used for
all allocations requested by Rust code in the same program.

## Allocator controls

The typed allocator controls are grouped by what they operate on rather than
mirroring jemalloc's entire namespace. Built-in keys are cached process-wide
after translation, and every subsequent control access uses `mallctlbymib`:

```rust
use jevmalloc::{Arena, ctl, stats};

fn main() -> Result<(), ctl::Error> {
    let arena = Arena::current()?;
    let decay = arena.muzzy_decay()?;
    arena.decay()?;
    let epoch = stats::refresh_epoch()?;

    println!("muzzy decay: {decay} ms, refreshed epoch: {epoch}");
    Ok(())
}
```

`Arena` represents one borrowed or explicitly created arena and exposes its
reclamation, decay, name, DSS, retained-growth, extent-hook, reset, and destroy
controls as methods. An arena created with `Arena::create` owns its lifecycle
and performs a best effort destroy when dropped. Allocation routing, reset,
explicit destruction, and custom hooks are unsafe because jemalloc cannot prove
that allocations, caches, callbacks, or thread associations have ended.
Constructing an unpinned handle from a raw arena index is likewise unsafe
because the caller must synchronize it with destruction and index recycling.

Allocator-wide queries, future-arena defaults, and the all-arenas reclamation
commands live under `jevmalloc::arenas`. Thread controls live under
`jevmalloc::this_thread`; arena operations resolve an `arena.0.*` template and
replace the numeric component with `thread.arena` before delegating to the same
instance methods.

Epoch refresh is explicit. Ordinary configuration and thread queries read live
state and do not force a process-wide statistics refresh. With the `stats`
feature, `jevmalloc::this_thread::ThreadCounters` provides repeated direct reads
of the calling thread's allocation counters without exposing them as immutable
static references.

`jevmalloc::ctl::raw` resolves ad hoc names and exposes unsafe generic MIB
reads, writes, mixed-type updates, exchanges, and commands. Value operations
require the Rust type to match the selected C control exactly, while commands
require the caller to uphold their semantic preconditions. Prefer a typed
crate-level control when one exists.

Profiling controls live under `jevmalloc::profiling` with the `profiling`
feature. Compiling that support does not activate profiling; jemalloc must also
start with `prof:true` in its allocator configuration. Epoch operations live
under `jevmalloc::stats`. Counter handles and peak controls live under
`jevmalloc::this_thread`, and mutex-statistics reset is present under
`jevmalloc::stats` with the `stats` feature.

## Symbol prefixing

The `unprefixed_malloc_on_supported_platforms` feature, on by default, builds
`jemalloc` without a symbol prefix, so it also takes over the C names and
services allocations made inside libc (`strdup`, `realpath(.., NULL)`, ...) and
by linked C++ (`operator new`, which libstdc++ implements over `malloc`). The
whole process then has one allocator.

Turning it off, or building for one of the targets in
`NO_UNPREFIXED_MALLOC_TARGETS`, prefixes every symbol with `_rjem_` and leaves
libc its own heap. Two allocators then coexist, and a pointer must be freed
through the same one that produced it.

`jevmalloc-sys/tests/single_allocator.rs` asserts whichever of the two is in
force, using `mallctl("arenas.lookup")` as the ownership oracle.

## Platform support

* `build`: does the library compile for the target?
* `run`: do the `jevmalloc` and `jevmalloc-sys` test suites pass on the target?
* `jemalloc`: does `jemalloc`'s own test suite pass on the target
  (`JEMALLOC_SYS_RUN_JEMALLOC_TESTS=1`)?

Every ✓ and ✗ below is measured by a CI cell. `?` marks a combination no cell
runs, so nothing here is claimed about it.

| Linux targets:                      | build     | run     | jemalloc     |
|-------------------------------------|-----------|---------|--------------|
| `aarch64-unknown-linux-gnu`         | ✓         | ✓       | ✓            |
| `x86_64-unknown-linux-gnu`          | ✓         | ✓       | ✓            |
| `x86_64-unknown-linux-musl`         | ✓         | ✓       | ?            |
| **MacOSX targets:**                 | **build** | **run** | **jemalloc** |
| `aarch64-apple-darwin`              | ✓         | ✓       | ?            |

## Features

`jevmalloc` re-exports every `jevmalloc-sys` feature; see
[`jevmalloc-sys/README.md`](jevmalloc-sys/README.md#features) for what each one
passes to `configure`. The default set is `cache_oblivious`,
`initial_exec_tls` and `unprefixed_malloc_on_supported_platforms`.

`jevmalloc` adds `global_hooks`, which calls a user-supplied hook (see
`jevmalloc::global::hook`) before entering `jemalloc` on each
`GlobalAlloc` operation.

## Testing

The table above is what CI measures, and it measures it through
[`docker/`](docker/README.md). Every CI job is one bake target plus a few
environment variables, so any of them reproduces locally:

```shell
./docker/bake.sh test                                    # the default regime
feat_set=prefixed ./docker/bake.sh test                  # the other symbol regime
feat_set=none ./docker/bake.sh valgrind                  # tests under Memcheck
feat_set=all cargo_profile=release ./docker/bake.sh suite # jemalloc's own suite
```

`docker/README.md` explains the axes, which of them move the C build, and why
no build artifact is cached.

## Benchmarks

The roundtrip benchmarks need the unstable `test` harness, so they are gated
behind `--cfg bench` and are not built by an ordinary `cargo build`:

```shell
RUSTFLAGS='--cfg bench' cargo +nightly bench -p jevmalloc -- <filter>
```

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `jevmalloc` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[ci]: https://github.com/matrix-construct/jevmalloc/actions/workflows/main.yml/badge.svg
[github actions]: https://github.com/matrix-construct/jevmalloc/actions

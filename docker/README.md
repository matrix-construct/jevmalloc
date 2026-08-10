# The build system

CI runs nothing that you cannot run here. Every job in
[`.github/workflows/main.yml`](../.github/workflows/main.yml) is one bake target
from [`bake.hcl`](bake.hcl) plus a handful of environment variables, so a red
cell reproduces with one command:

```shell
feat_set=all cargo_profile=release ./docker/bake.sh suite
```

The workflow files hold no build logic. They choose a machine, name the cells,
and call [`bake.sh`](bake.sh).

## Targets

| Target   | What it runs                                                        |
|----------|---------------------------------------------------------------------|
| `build`  | `cargo build --workspace`                                            |
| `test`   | `cargo test --workspace`, so unit, integration and doc tests         |
| `valgrind` | the same tests under Memcheck, continuing through every target      |
| `clippy` | `cargo clippy --all-targets`, denying warnings                       |
| `fmt`    | `cargo fmt --check`, pinned to nightly by `rustfmt.toml`             |
| `doc`    | `cargo doc`, with the `jevmalloc_docs` cfg the binding shapes need   |
| `bench`  | type-checks `benches/`, which nothing else compiles (see below)      |
| `suite`  | jemalloc's own suite, about 1800 cases, via `make check`             |

There are three groups: `lint` (`fmt`, `clippy`, `doc`, `bench`), `tests`
(`test`, `valgrind`, `suite`), and `default`, which is both. With no target you
get `default`, on whatever axes are set.

The gating Valgrind cell uses `feat_set=none`, the prefixed symbol regime, and
passes `--no-fail-fast` so one report cannot hide later test binaries. The
dedicated tools layer pins `cargo-valgrind` 2.4.1, whose standard-library
suppressions cover the v0-mangled `std::thread::current` allocation used by
current Rust test harnesses. `VALGRINDFLAGS` leaves program-defined allocator
symbols unintercepted, so Valgrind cannot replace only the unprefixed half of a
statically linked jemalloc build.

## Axes

Each takes one value from the environment, or a JSON array through its plural
form (`feat_sets='["all","none"]'`) to widen it locally.

| Variable         | Values                                                    |
|------------------|-----------------------------------------------------------|
| `feat_set`       | `default`, `stats`, `prefixed`, `none`, `all`             |
| `cargo_profile`  | `dev`, `release`                                          |
| `cc_name`        | `gcc`, `clang`                                            |
| `rust_toolchain` | `stable`, `nightly`                                       |
| `rust_target`    | any installed target; defaults to the host                |
| `sys_name`       | `debian`                                                  |

Three of these move the C build, which is the point of matrixing them:

* **`feat_set`.** Every jemalloc feature passes an explicit `--enable-X` or
  `--disable-X` to `configure`, so `all` and `none` are the two extremes of the
  option spread rather than a default and a superset. `default` and `stats`
  link jemalloc unprefixed, where it also services libc's own allocations;
  `prefixed` and `none` drop the default features, which is what asks for
  `--with-jemalloc-prefix=_rjem_` and sets the `prefixed` cfg. Those are two
  distinct links, and `jevmalloc-sys` has tests that compile under only one or
  only the other, so both regimes have to run rather than build.
* **`cargo_profile`.** There is no `debug` cargo feature: `build.rs` reads
  `debug_assertions`, so `dev` configures jemalloc with `--enable-debug` and
  `release` with `--disable-debug` and `-DNDEBUG`. They are disjoint C builds.
* **`cc_name`.** Selects the compiler `build.rs` hands to `cc`, as
  `CC_<target>`. On a musl target `gcc` resolves to the `musl-gcc` wrapper.

A musl leaf also picks up [`.cargo/config.toml`](../.cargo/config.toml), which
names `-lc` a second time at the end of the link line. rustc places the standard
library's own `-lc` ahead of the bundled jemalloc objects and nothing puts
another after them, so without it every musl test binary fails to link against
the whole libc surface jemalloc touches.

`JEMALLOC_SYS_WITH_MALLOC_CONF`, `JEMALLOC_SYS_WITH_LG_PAGE`,
`JEMALLOC_SYS_WITH_LG_HUGEPAGE`, `JEMALLOC_SYS_WITH_LG_QUANTUM` and
`JEMALLOC_SYS_WITH_LG_VADDR` pass straight through under their real names, so a
bake leaf and a local `cargo` run take the same environment. None of them is
set by a gating cell: jemalloc's suite is not known to pass under a non-default
page or quantum, and a lowered quantum is under-alignment UB that
`jevmalloc/tests/flags.rs` exists to catch. Reach them through the workflow's
manual dispatch, or here:

```shell
JEMALLOC_SYS_WITH_MALLOC_CONF=background_thread:true ./docker/bake.sh suite
```

## What gets cached, and what deliberately does not

The layers are `system` (the distribution and every C toolchain), `rustup`,
`rust` (the toolchain and its components), `source`, then one leaf per cell.
Caching stops at `rust`. That layer is the expensive, slow-moving one, and it is
shared by every leaf; the cargo registry, the rustup downloads and apt's own
downloads ride along in cache mounts.

**No leaf shares a `CARGO_TARGET_DIR` with another, and none is cached.** A cold
C build is around 25 seconds, so there is little to win, and quite a lot to
lose: `build.rs` deliberately does not watch `JEMALLOC_SYS_RUN_JEMALLOC_TESTS`,
so a `suite` leaf that inherited a warm target dir would take the cache hit and
report a pass without running a single case. The whole matrix is only
meaningful if each cell configures and compiles jemalloc itself.

`source` is an allowlist of the files that actually feed a build, so editing
this README, a workflow, or `bake.hcl` invalidates no cargo layer.

The builder is created on first use with a garbage-collection policy that keeps
build layers for 6 hours under an 8 GB ceiling, about one full matrix, so the
toolchain layers every leaf references stay warm while the per-leaf ones are
what gets trimmed. buildkit only reclaims a record
that is both over the ceiling and older than its keep duration, so a generous
duration would shield the whole cache, leave the ceiling inert, and grow until
the disk-pressure valve dumped everything at once. Tune with `layer_max_space`
and friends; the policy applies at creation, so change it with
`docker buildx rm jevmalloc` and a re-run.

## Machines

The x64 self-hosted pool is a single shared machine that also carries Tuwunel's
CI, so cells are capped in flight rather than fanned out as wide as they will
go, and a superseded run on a branch other than `main` is cancelled.

Self-hosted runners are reachable only from pushes and manual dispatches. A
pull request can carry a fork's code, so those runs stay on GitHub-hosted
machines. Nothing about the build changes; only the machine does.

Darwin is the one leg outside all of this, because there is no macOS docker
host to bake on. It builds natively in the workflow and matches what the
support table in the top-level [README](../README.md) claims for it.

## Combinations the matrix deliberately avoids

Found by running the matrix, and the reason two cells are not the obvious ones:

* **`feat_set=all` on musl does not build.** `profiling` reaches
  `src/prof_sys.c`, which includes `<execinfo.h>` under
  `JEMALLOC_PROF_FRAME_POINTER` as its fallback unwinder, and musl has no
  `execinfo.h`. `profiling_frameptr` does not rescue it. The musl cell
  therefore uses `stats`.
* **`feat_set=none` under `dev` fails jemalloc's suite.**
  `test/unit/double_free` exits with a status the harness does not recognise,
  reported as `Test harness error`, once `--enable-debug` sits on top of
  everything-disabled. The `none` suite cell therefore runs under `release`,
  where it is clean. The `test` target runs `none` under both.

Neither is carried as a permanently failing report-only cell: a red that never
changes teaches people to ignore reds. They are written down here instead.

## Cells that report without gating

The aarch64 jemalloc suite and the Darwin test run carry `soft`, which reports
their result without failing the run. Both pass today, and the support table
says so; they stay report-only because they are the two legs nobody can
reproduce from a development machine here, so a red in either is a thing to go
and look at rather than a thing to block on.

`soft` sits on the step and not on the job, which matters more than it sounds.
A job carrying `continue-on-error` still reports its check run as failed, and
the commit list aggregates check runs rather than the run's own conclusion, so
that spelling hangs a red X on a commit whose run reads green. Failing the step
keeps the job green; a `Report` step then raises a warning annotation, which is
what carries the result up to the run summary.

Two combinations in the support table have no cell at all: the jemalloc suite
on musl and on Darwin. They are marked `?` rather than ✗, so the table claims
only what it measures.

## Benches

`benches/roundtrip.rs` is `#![cfg(bench)]`, so without `RUSTFLAGS='--cfg bench'`
it compiles to an empty crate and `clippy --all-targets` type-checks nothing in
it. The `bench` target is the only leaf that passes that cfg, and it is why the
benches cannot rot unnoticed. Its harness needs nightly.

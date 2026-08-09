###############################################################################
#
# The build recipe. Every CI job is one bake target; the workflow only chooses
# which, and on what axes. Run any of them locally with the same arguments CI
# uses, e.g.
#
#     feat_set=all cargo_profile=release docker/bake.sh test
#
# See docker/README.md for the axes and their values.
#

variable "docker_dir" {
    default = "docker"
}

variable "nprocs" {
    default = 1
}

#
# Axes. Each is a JSON array, so `docker/bake.sh` can widen one from the
# environment without editing this file, and the workflow can hand a single
# value per job for a one-cell fan-out.
#

variable "sys_names" {
    default = "[\"debian\"]"
}
variable "cc_names" {
    default = "[\"gcc\"]"
}
variable "rust_toolchains" {
    default = "[\"stable\"]"
}
variable "rust_targets" {
    default = "[\"x86_64-unknown-linux-gnu\"]"
}
variable "cargo_profiles" {
    default = "[\"dev\"]"
}
variable "feat_sets" {
    default = "[\"default\"]"
}

sys_versions = {
    debian = "trixie-slim"
}

# The toolchain each axis value resolves to. `stable` and `nightly` are moving
# targets; overriding either from the environment pins a CI run to a known
# toolchain when a release breaks us.
variable "rust_msrv" {
    default = "stable"
}
variable "rust_nightly" {
    default = "nightly"
}

# The host the rustup installer is fetched for. bake.sh derives this from the
# machine it runs on; every leg builds natively, nothing cross-builds the host.
variable "host_triple" {
    default = "x86_64-unknown-linux-gnu"
}

# build.rs turns each of these into a configure flag and watches it, so setting
# one here is enough to get a distinct C build. They keep their real names so a
# local `cargo` run and a bake leaf take the same environment. Empty is the
# normal case; the workflow only sets them for a manual probe, since none of
# jemalloc's own suite is known to pass under a non-default page or quantum.
variable "JEMALLOC_SYS_WITH_MALLOC_CONF" {
    default = ""
}
variable "JEMALLOC_SYS_WITH_LG_PAGE" {
    default = ""
}
variable "JEMALLOC_SYS_WITH_LG_HUGEPAGE" {
    default = ""
}
variable "JEMALLOC_SYS_WITH_LG_QUANTUM" {
    default = ""
}
variable "JEMALLOC_SYS_WITH_LG_VADDR" {
    default = ""
}

#
# Feature regimes. `default` and `stats` link jemalloc unprefixed and let it
# service libc's allocations too; `none` and `prefixed` drop the default
# features, which is what emits --with-jemalloc-prefix=_rjem_ and the
# `prefixed` cfg. Those are two distinct links, and jevmalloc-sys has tests
# that only compile under one or the other, so both must run.
#
cargo_feat_sets = {
    none     = "--no-default-features"
    default  = ""
    stats    = "--features stats"
    prefixed = "--no-default-features --features stats"
    all      = "--all-features"
}

# There is no `debug` cargo feature: build.rs reads debug_assertions, so the
# cargo profile is itself a C-build axis. `dev` configures jemalloc with
# --enable-debug, `release` with --disable-debug and -DNDEBUG.
cargo_profile_args = {
    dev     = ""
    release = "--release"
}

###############################################################################
#
# Base layers
#

# One system layer holds every compiler, so cc_name and the musl leg cost a
# leaf argument rather than a distinct base image.
target "system" {
    description = "Base system with the C toolchains jemalloc's configure needs."
    name = elem("system", [sys_name])
    tags = [elem_tag("system", [sys_name], "latest")]
    target = "system"
    dockerfile = "${docker_dir}/Dockerfile.system"
    context = "."
    output = ["type=cacheonly"]
    matrix = {
        sys_name = jsondecode(sys_names)
    }
    args = {
        sys_name = sys_name
        sys_version = sys_versions[sys_name]
        packages = join(" ", [
            "ca-certificates",
            "clang",
            "curl",
            "gcc",
            "libc6-dev",
            "make",
            "musl-tools",
        ])
    }
}

target "rustup" {
    description = "The rustup installer, fetched once per system."
    name = elem("rustup", [sys_name])
    tags = [elem_tag("rustup", [sys_name], "latest")]
    target = "rustup"
    dockerfile = "${docker_dir}/Dockerfile.rust"
    output = ["type=cacheonly"]
    matrix = {
        sys_name = jsondecode(sys_names)
    }
    inherits = [elem("system", [sys_name])]
    contexts = {
        input = elem("target:system", [sys_name])
    }
    args = {
        host_triple = host_triple
        RUST_HOME = "/opt/rust"
    }
}

target "rust" {
    description = "The toolchain layer every leaf shares."
    name = elem("rust", [sys_name, rust_toolchain, rust_target])
    tags = [elem_tag("rust", [sys_name, rust_toolchain, rust_target], "latest")]
    target = "rust"
    dockerfile = "${docker_dir}/Dockerfile.rust"
    output = ["type=cacheonly"]
    # Widened past the requested axes: fmt, doc and the bench check pin
    # themselves to nightly on the host, so those layers must exist however
    # narrow the job's own axes are. Unreferenced ones are never built.
    matrix = {
        sys_name = jsondecode(sys_names)
        rust_toolchain = distinct(concat(jsondecode(rust_toolchains), ["nightly"]))
        rust_target = distinct(concat(jsondecode(rust_targets), [host_triple]))
    }
    inherits = [elem("rustup", [sys_name])]
    contexts = {
        input = elem("target:rustup", [sys_name])
    }
    args = {
        host_triple = host_triple
        rust_target = rust_target
        rust_toolchain = toolchain(rust_toolchain)
        rustup_components = join(" ", ["clippy", "rustfmt"])
        RUST_HOME = "/opt/rust"
        RUSTUP_HOME = "/opt/rust/rustup"
        CARGO_HOME = "/opt/rust/cargo"
    }
}

###############################################################################
#
# Leaves. Each runs one cargo command and exports nothing; the exit status is
# the result.
#

# `cargo` and `source` are deliberately absent: they are the shared scaffolding
# the leaves build on, not things to run, and `cargo` has no input context of
# its own.
group "default" {
    targets = ["lint", "tests"]
}

group "lint" {
    targets = ["bench", "clippy", "doc", "fmt"]
}

group "tests" {
    targets = ["suite", "test"]
}

# Not buildable on its own: the concrete leaves below fill in cargo_cmd and
# cargo_args. Holds everything they share.
target "cargo" {
    target = "cargo"
    dockerfile = "${docker_dir}/Dockerfile.cargo"
    context = "."
    output = ["type=cacheonly"]
    contexts = {
        source = "target:source"
    }
    args = {
        nprocs = nprocs
        RUSTUP_HOME = "/opt/rust/rustup"
        CARGO_HOME = "/opt/rust/cargo"
    }
}

target "source" {
    description = "The source allowlist, so a docs or CI edit invalidates nothing."
    target = "source"
    dockerfile = "${docker_dir}/Dockerfile.cargo"
    context = "."
    output = ["type=cacheonly"]
}

target "build" {
    description = "Compile the workspace."
    name = elem("build", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set])
    tags = [elem_tag("build", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set], "latest")]
    matrix = full
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, rust_toolchain, rust_target])
    }
    args = merge(leaf_args(sys_name, cc_name, rust_toolchain, rust_target), {
        cargo_cmd = "build"
        cargo_args = cargo_workspace_args(rust_target, cargo_profile, feat_set)
    })
}

target "test" {
    description = "Unit, integration and doc tests."
    name = elem("test", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set])
    tags = [elem_tag("test", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set], "latest")]
    matrix = full
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, rust_toolchain, rust_target])
    }
    args = merge(leaf_args(sys_name, cc_name, rust_toolchain, rust_target), {
        cargo_cmd = "test"
        cargo_args = cargo_workspace_args(rust_target, cargo_profile, feat_set)
    })
}

target "clippy" {
    description = "Lints, denied."
    name = elem("clippy", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set])
    tags = [elem_tag("clippy", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set], "latest")]
    matrix = full
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, rust_toolchain, rust_target])
    }
    args = merge(leaf_args(sys_name, cc_name, rust_toolchain, rust_target), {
        cargo_cmd = "clippy"
        cargo_args = join(" ", [
            cargo_workspace_args(rust_target, cargo_profile, feat_set),
            "--all-targets --no-deps -- -D warnings",
        ])
    })
}

# The benches are stripped to an empty crate without --cfg bench, so
# `clippy --all-targets` above type-checks nothing in them. This is the only
# leaf that compiles them, and the `test` harness they use needs nightly.
target "bench" {
    description = "Type-check the benches, which no other leaf compiles."
    name = elem("bench", [sys_name, rust_target])
    tags = [elem_tag("bench", [sys_name, rust_target], "latest")]
    matrix = {
        sys_name = jsondecode(sys_names)
        rust_target = jsondecode(rust_targets)
    }
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, "nightly", rust_target])
    }
    args = merge(leaf_args(sys_name, "gcc", "nightly", rust_target), {
        cargo_cmd = "check"
        cargo_args = "-p jevmalloc --benches --all-features --target ${rust_target}"
        rustflags = "--cfg bench"
    })
}

# rustfmt.toml sets unstable options, so this cannot run on stable.
target "fmt" {
    description = "Formatting."
    name = elem("fmt", [sys_name])
    tags = [elem_tag("fmt", [sys_name], "latest")]
    matrix = {
        sys_name = jsondecode(sys_names)
    }
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, "nightly", host_triple])
    }
    args = merge(leaf_args(sys_name, "gcc", "nightly", host_triple), {
        cargo_cmd = "fmt"
        cargo_args = "--all -- --check"
    })
}

target "doc" {
    description = "Documentation."
    name = elem("doc", [sys_name, rust_target])
    tags = [elem_tag("doc", [sys_name, rust_target], "latest")]
    matrix = {
        sys_name = jsondecode(sys_names)
        rust_target = jsondecode(rust_targets)
    }
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, "nightly", rust_target])
    }
    args = merge(leaf_args(sys_name, "gcc", "nightly", rust_target), {
        cargo_cmd = "doc"
        cargo_args = "--workspace --all-features --no-deps --target ${rust_target}"
    })
}

# jemalloc's own suite (~1800 cases). build.rs runs it from the build script, so
# this is a `cargo build` whose real work is `make check`. It deliberately does
# not watch JEMALLOC_SYS_RUN_JEMALLOC_TESTS, which is why no leaf shares a
# target dir with another.
target "suite" {
    description = "jemalloc's own test suite."
    name = elem("suite", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set])
    tags = [elem_tag("suite", [sys_name, cc_name, rust_toolchain, rust_target, cargo_profile, feat_set], "latest")]
    matrix = full
    inherits = ["cargo"]
    contexts = {
        input = elem("target:rust", [sys_name, rust_toolchain, rust_target])
    }
    args = merge(leaf_args(sys_name, cc_name, rust_toolchain, rust_target), {
        cargo_cmd = "build"
        cargo_args = join(" ", [
            "-p jevmalloc-sys",
            "--target ${rust_target}",
            cargo_profile_args[cargo_profile],
            cargo_feat_sets[feat_set],
        ])
        run_jemalloc_tests = "1"
    })
}

###############################################################################
#
# Utils
#

full = {
    sys_name = jsondecode(sys_names)
    cc_name = jsondecode(cc_names)
    rust_toolchain = jsondecode(rust_toolchains)
    rust_target = jsondecode(rust_targets)
    cargo_profile = jsondecode(cargo_profiles)
    feat_set = jsondecode(feat_sets)
}

function "leaf_args" {
    params = [sys_name, cc_name, rust_toolchain, rust_target]
    result = {
        sys_name = sys_name
        cc_name = cc_name
        cc_bin = cc_bin(cc_name, rust_target)
        cc_env_name = "CC_${replace(rust_target, "-", "_")}"
        rust_target = rust_target
        rust_toolchain = toolchain(rust_toolchain)

        malloc_conf = JEMALLOC_SYS_WITH_MALLOC_CONF
        lg_page = JEMALLOC_SYS_WITH_LG_PAGE
        lg_hugepage = JEMALLOC_SYS_WITH_LG_HUGEPAGE
        lg_quantum = JEMALLOC_SYS_WITH_LG_QUANTUM
        lg_vaddr = JEMALLOC_SYS_WITH_LG_VADDR
    }
}

function "cargo_workspace_args" {
    params = [rust_target, cargo_profile, feat_set]
    result = join(" ", [
        "--workspace",
        "--target ${rust_target}",
        cargo_profile_args[cargo_profile],
        cargo_feat_sets[feat_set],
    ])
}

# musl is a distinct libc, not a distinct compiler: gcc reaches it through the
# musl-gcc wrapper. clang against musl is not wired up and is excluded by the
# workflow rather than silently falling back to glibc here.
function "cc_bin" {
    params = [cc_name, rust_target]
    result = (
        cc_name == "clang"?
            "clang":
        length(regexall("-musl$", rust_target)) > 0?
            "musl-gcc":
            "gcc"
    )
}

function "toolchain" {
    params = [rust_toolchain]
    result = (
        rust_toolchain == "stable"?
            rust_msrv:
        rust_toolchain == "nightly"?
            rust_nightly:
            rust_toolchain
    )
}

function "elem_tag" {
    params = [prefix, matrix, tag]
    result = join(":", [elem(prefix, matrix), tag])
}

function "elem" {
    params = [prefix, matrix]
    result = join("--", [prefix, join("--", matrix)])
}

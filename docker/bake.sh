#!/bin/bash
# Thin driver over docker/bake.hcl. The workflow calls this and nothing else, so
# any CI cell reproduces locally by exporting the same axes:
#
#     ./docker/bake.sh fmt
#     feat_set=prefixed cargo_profile=release ./docker/bake.sh test
#     feat_set=none ./docker/bake.sh valgrind
#     cc_name=clang feat_set=all ./docker/bake.sh suite
#
# With no target it builds the `default` group, which is lint plus tests, on
# whatever axes are set.

set -eo pipefail

# Resolve the repository root from the script's own location and chdir there.
# The bake context and every path in bake.hcl are relative to it.
cd "$(dirname "$0")/.."
BASEDIR="docker"

# Every leg builds natively; nothing cross-builds the host, so the rustup
# installer and the default rust target both follow the machine we are on. This
# is what lets the same cell definition run on the x64 and arm64 pools.
case "$(uname -m)" in
    aarch64|arm64) host_arch="aarch64" ;;
    *) host_arch="x86_64" ;;
esac
export host_triple="${host_triple:-${host_arch}-unknown-linux-gnu}"

default_sys_names='["debian"]'
default_cc_names='["gcc"]'
default_rust_toolchains='["stable"]'
default_rust_targets="[\"${host_triple}\"]"
default_cargo_profiles='["dev"]'
default_feat_sets='["default"]'

# A singular env var pins its axis to one value, which is how the workflow hands
# each job exactly one cell. The plural form takes a JSON array to widen it.
if test -n "$sys_name"; then env_sys_names="[\"${sys_name}\"]"; fi
if test -n "$cc_name"; then env_cc_names="[\"${cc_name}\"]"; fi
if test -n "$rust_toolchain"; then env_rust_toolchains="[\"${rust_toolchain}\"]"; fi
if test -n "$rust_target"; then env_rust_targets="[\"${rust_target}\"]"; fi
if test -n "$cargo_profile"; then env_cargo_profiles="[\"${cargo_profile}\"]"; fi
if test -n "$feat_set"; then env_feat_sets="[\"${feat_set}\"]"; fi

set -a
bake_target="${bake_target:-$*}"
sys_names="${sys_names:-${env_sys_names:-$default_sys_names}}"
cc_names="${cc_names:-${env_cc_names:-$default_cc_names}}"
rust_toolchains="${rust_toolchains:-${env_rust_toolchains:-$default_rust_toolchains}}"
rust_targets="${rust_targets:-${env_rust_targets:-$default_rust_targets}}"
cargo_profiles="${cargo_profiles:-${env_cargo_profiles:-$default_cargo_profiles}}"
feat_sets="${feat_sets:-${env_feat_sets:-$default_feat_sets}}"

docker_dir="$BASEDIR"

# `stable` and `nightly` float. Pin either here to hold a run steady across a
# toolchain release; a dated nightly is the usual reason.
rust_msrv="${rust_msrv:-stable}"
rust_nightly="${rust_nightly:-nightly}"

if test "$(uname)" = "Darwin"; then
    nprocs="${nprocs:-$(sysctl -n hw.logicalcpu)}"
else
    nprocs="${nprocs:-$(nproc)}"
fi
set +a

###############################################################################

# One builder per GitHub actor, shared with every other repository built on the
# same machine. Tuwunel addresses it by the same name, so on the self-hosted
# pool this resolves to the one builder both projects already use, with one
# buildkit, one layer cache and one garbage-collection policy between them. A
# second builder here would be a second full-fat cache competing for the same
# disk, which is the arrangement this replaced.
builder_name="${builder_name:-${GITHUB_ACTOR:-jevmalloc}}"

# On CI the builder and its garbage-collection policy are the init job's, from
# the shared .github/workflows/init.sh, and it exists before any cell runs. This
# is the fallback for the two cases init does not cover: a workstation, and the
# ephemeral GitHub-hosted machines that pull requests build on. Neither shares a
# disk with anything, so neither needs a policy, and deliberately configuring
# nothing here keeps init.sh the only thing that can define the shared builder.
#
# The self-hosted pool is many runner instances against one docker daemon and
# one buildx state directory, so several jobs can reach this concurrently.
# Whoever loses the race just adopts the winner's builder.
if ! docker buildx inspect "$builder_name" >/dev/null 2>&1; then
    docker buildx create \
        --name "$builder_name" \
        --driver docker-container \
        >/dev/null 2>&1 || true
fi

docker buildx inspect --bootstrap "$builder_name" >/dev/null

export DOCKER_BUILDKIT=1
if test "$CI" = "true"; then
    export BUILDKIT_PROGRESS="plain"
fi

args=""
args="$args --provenance=false"
args="$args --builder ${builder_name}"
args="$args -f ${BASEDIR}/bake.hcl"

if test "$CI_PRINT_BAKE" = "true"; then
    docker buildx bake --print $args $bake_target
fi

if test "$NO_BAKE" = "1"; then
    exit 0
fi

set -ux
docker buildx bake $args $bake_target
set +x
echo -e "\033[1;42;30mACCEPT\033[0m"

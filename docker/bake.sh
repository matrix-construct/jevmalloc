#!/bin/bash
# Thin driver over docker/bake.hcl. The workflow calls this and nothing else, so
# any CI cell reproduces locally by exporting the same axes:
#
#     ./docker/bake.sh fmt
#     feat_set=prefixed cargo_profile=release ./docker/bake.sh test
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

# The builder is ours alone. Sharing one with another repository would mean
# sharing its garbage-collection policy, and the whole point of the policy below
# is that this cache stays small enough for the ceiling to bind.
builder_name="${builder_name:-jevmalloc}"

# buildkit only reclaims a record that is over the ceiling *and* older than
# keepDuration, so a long keepDuration shields the whole cache, the ceiling
# never binds, and it grows until the disk-pressure rule dumps everything. A
# short duration on the build layers keeps the shielded set under the ceiling.
# Cache mounts (apt, the cargo registry, rustup downloads) are small and worth
# keeping, so they get their own bucket.
# A full matrix leaves roughly 8 GB of layer records, so the ceiling holds about
# one run: the toolchain layers every leaf references stay warm, and the per-leaf
# ones, which have to rebuild on any source change anyway, are what gets trimmed.
# Deliberately modest, since the shared machine has its own disk pressure.
layer_keep_duration="${layer_keep_duration:-6h}"
layer_max_space="${layer_max_space:-8GB}"
cachemount_keep_duration="${cachemount_keep_duration:-168h}"
cachemount_max_space="${cachemount_max_space:-2GB}"
min_free_space="${min_free_space:-16GB}"

# Applied at creation only; `docker buildx rm $builder_name` and re-run to
# change it.
buildkitd_config() {
    cat <<EOF
[worker.oci]
  gc = true

  [[worker.oci.gcpolicy]]
    filters = ["type==exec.cachemount"]
    keepDuration = "${cachemount_keep_duration}"
    maxUsedSpace = "${cachemount_max_space}"

  [[worker.oci.gcpolicy]]
    filters = ["type!=exec.cachemount"]
    keepDuration = "${layer_keep_duration}"
    maxUsedSpace = "${layer_max_space}"

  [[worker.oci.gcpolicy]]
    all = true
    minFreeSpace = "${min_free_space}"
EOF
}

# The self-hosted pool is many runner instances against one docker daemon and
# one buildx state directory, so a first run can have several jobs creating this
# concurrently. Whoever loses the race just adopts the winner's builder.
if ! docker buildx inspect "$builder_name" >/dev/null 2>&1; then
    config="$(mktemp)"
    trap 'rm -f "$config"' EXIT
    buildkitd_config > "$config"
    docker buildx create \
        --name "$builder_name" \
        --driver docker-container \
        --config "$config" \
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

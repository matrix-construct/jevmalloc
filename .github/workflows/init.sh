#!/bin/bash

set +e

builder="${GITHUB_ACTOR}"
seed_builder="${seed_builder:-jevolk}"

clean=
nocache=
case "$pipeline" in
*"[ci clean nocache]"*) clean=1; nocache=1 ;;
*"[ci clean-rust]"*)    clean=1; nocache=1 ;;
*"[ci clean]"*)         clean=1 ;;
esac

avail_bytes() {
	df -PB1 /var/lib/docker | awk 'NR==2 {print $4}'
}

gib() {
	echo $(( ${1%%[!0-9]*} * 1024 * 1024 * 1024 ))
}

builders_with_state() {
	docker volume ls -q 2>/dev/null \
	| sed -n 's/^buildx_buildkit_\(.*\)0_state$/\1/p'
}

seen_vol="tuwunel_ci_seen"
docker volume create "$seen_vol" >/dev/null 2>&1

mark_seen() {
	docker run --rm -v "${seen_vol}:/seen" busybox \
		touch "/seen/$1" >/dev/null 2>&1
}

mark_seen "$builder"

# Runner-keyed knobs (JSON maps of runner -> value, selected by $runner).
reserved_space=$(echo -n "$reserved_space" | jq -r ".$runner")
max_used_space=$(echo -n "$max_used_space" | jq -r ".$runner")
cachemount_max=$(echo -n "$cachemount_max" | jq -r ".$runner")
min_free_space=$(echo -n "$min_free_space" | jq -r ".$runner")
safety_free_space=$(echo -n "$safety_free_space" | jq -r ".$runner")
reap_idle_hours=$(echo -n "$reap_idle_hours" | jq -r ".$runner")
reap_min_free=$(echo -n "$reap_min_free" | jq -r ".$runner")
seed_budget=$(echo -n "$seed_budget" | jq -r ".$runner")

reap_builder() {
	docker buildx rm "$1" >/dev/null 2>&1
	docker volume rm -f "buildx_buildkit_${1}0_state" >/dev/null 2>&1
	docker run --rm -v "${seen_vol}:/seen" busybox \
		rm -f "/seen/$1" >/dev/null 2>&1

	echo "reaped idle builder: $1"
}

now=$(date +%s)
reap_idle_secs=$(( reap_idle_hours * 3600 ))
markers=$(docker run --rm -v "${seen_vol}:/seen" busybox sh -c '
	cd /seen 2>/dev/null || exit 0
	for f in *; do
		[ -e "$f" ] &&
			echo "$(stat -c %Y "$f") $f"
	done
' 2>/dev/null)

marker_epoch() {
	echo "$markers" | awk -v n="$1" '$2 == n {print $1; exit}'
}

for name in $(builders_with_state); do
	test "$name" = "$seed_builder" && continue
	test "$name" = "$builder" && continue

	epoch=$(marker_epoch "$name")
	if test -z "$epoch"; then
		# First sighting of a pre-existing builder: grant it a grace
		# period rather than reap something that may be in active use.
		mark_seen "$name"
		continue
	fi

	test $(( now - epoch )) -gt "$reap_idle_secs" && reap_builder "$name"
done

free_floor=$(gib "$reap_min_free")
if test "$(avail_bytes)" -lt "$free_floor"; then
	min_idle_secs=$(( 12 * 3600 ))
	for name in $(builders_with_state); do
		test "$name" = "$seed_builder" && continue
		test "$name" = "$builder" && continue
		epoch=$(marker_epoch "$name")
		test -z "$epoch" && continue
		echo "$(( now - epoch )) $name"
	done \
	| sort -rn \
	| while read -r builder_idle_secs name; do
		test -n "$name" || continue
		test "$builder_idle_secs" -gt "$min_idle_secs" || continue
		test "$(avail_bytes)" -ge "$free_floor" && break
		reap_builder "$name"
	done
fi

if test -n "$clean"; then
	docker buildx rm "$builder"
fi

docker buildx inspect "$builder"
if test x"$?" = x"0"; then
	exit 0
fi

set -eux

cat <<EOF > ./buildkitd.toml
[system]
  platformsCacheMaxAge = "504h"
[worker.oci]
  enabled = true
  rootless = false
  gc = true
  reservedSpace = "${reserved_space}"
  maxUsedSpace = "${max_used_space}"
  minFreeSpace = "${min_free_space}"

[[worker.oci.gcpolicy]]
  filters = ["type==exec.cachemount"]
  keepDuration = "336h"
  maxUsedSpace = "${cachemount_max}"

[[worker.oci.gcpolicy]]
  filters = ["type!=exec.cachemount"]
  keepDuration = "12h"
  reservedSpace = "${reserved_space}"
  maxUsedSpace = "${max_used_space}"
  all = true

[[worker.oci.gcpolicy]]
  minFreeSpace = "${safety_free_space}"
  all = true
EOF

seed_state="buildx_buildkit_${seed_builder}0_state"
this_state="buildx_buildkit_${builder}0_state"
seeded=
exec 200>/tmp/tuwunel-ci-seed.lock
flock -x 200 || true
if test -z "$nocache" \
	&& test "$builder" != "$seed_builder" \
	&& test "$(avail_bytes)" -ge "$(gib "$seed_budget")" \
	&& docker volume inspect "$seed_state" >/dev/null 2>&1
then
	docker volume create "$this_state"

	docker run --rm \
		-v "${seed_state}:/seed:ro" \
		-v "${this_state}:/state" \
		busybox sh -c 'cp -a /seed/. /state/' || true
	seeded=1
fi
flock -u 200 || true

create_builder() {
	docker buildx create \
		--bootstrap \
		--driver docker-container \
		--buildkitd-config ./buildkitd.toml \
		--name "$builder" \
		--buildkitd-flags "--allow-insecure-entitlement network.host"
}

if ! create_builder; then
	if test -n "$seeded"; then
		docker buildx rm "$builder" || true
		docker volume rm -f "$this_state" || true
		create_builder
	else
		exit 1
	fi
fi

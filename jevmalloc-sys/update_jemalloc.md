# Updating jemalloc

The C source is the `jemalloc` submodule; the `configure` script `build.rs`
feeds it is **not** taken from that submodule, but from the checked-in copy in
`configure/`. Moving the submodule without regenerating that copy silently
keeps building the old configuration, so the two steps go together.

Generating `configure` requires `autoconf` to be installed.

1. Advance the submodule to the new source revision:

```shell
git -C jemalloc fetch origin
git -C jemalloc merge --ff-only origin/master
git add jemalloc
```

2. Regenerate `configure` from the new `configure.ac` and install it, leaving
   the submodule's working tree clean (`jemalloc/configure` is gitignored
   there, but `build.rs` copies the whole directory into `OUT_DIR`):

```shell
(cd jemalloc && autoconf)
cp jemalloc/configure configure/configure
rm jemalloc/configure
```

3. Update the workspace `version` build metadata in the root `Cargo.toml` to
   the new source revision. `build.rs` splits that string on `+` and passes the
   remainder to `--with-version`, so it must stay in jemalloc's
   `<major>.<minor>.<bugfix>-<nrev>-g<gid>` form; `git -C jemalloc describe
   --long` produces it (with the abbreviated gid expanded to the full hash).

4. Diff the option set and reconcile `build.rs`, which passes an explicit
   `--enable-`/`--disable-` pair for every feature it exposes:

```shell
git -C jemalloc diff <old-rev> HEAD -- configure.ac \
  | grep -E '^[+-].*(AC_ARG_ENABLE|AC_ARG_WITH)'
```

   An option that disappeared breaks the build immediately; one that appeared
   is only a missed opportunity, so review the additions by hand.

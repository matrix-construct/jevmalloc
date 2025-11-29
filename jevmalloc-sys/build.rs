// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{
	env,
	ffi::OsString,
	fs, io,
	path::{Path, PathBuf},
	process::Command,
};

use rustflags::Flag as RustFlag;

include!("src/env.rs");

macro_rules! info {
    ($($args:tt)*) => { println!($($args)*) }
}

macro_rules! warning {
    ($arg:tt, $($args:tt)*) => {
        println!(concat!(concat!("cargo:warning=\"", $arg), "\""), $($args)*)
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
fn main() {
	let target = expect_env("TARGET");
	let host = expect_env("HOST");
	let num_jobs = expect_env("NUM_JOBS");
	let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR was not set"));
	let src_dir = env::current_dir().expect("failed to get current directory");
	let version = expect_env("CARGO_PKG_VERSION");
	let je_version = version
		.split_once('+')
		.expect("jemalloc version is missing")
		.1;

	info!("TARGET={}", target);
	info!("HOST={}", host);
	info!("NUM_JOBS={}", num_jobs);
	info!("OUT_DIR={:?}", out_dir);
	let build_dir = out_dir.join("build");
	info!("BUILD_DIR={:?}", build_dir);
	info!("SRC_DIR={:?}", src_dir);
	info!("ENV={:?}", env::vars());

	if UNSUPPORTED_TARGETS
		.iter()
		.any(|i| target.contains(i))
	{
		panic!("jemalloc does not support target: {target}");
	}

	if UNTESTED_TARGETS
		.iter()
		.any(|i| target.contains(i))
	{
		warning!("jemalloc support for `{}` is untested", target);
	}

	let mut use_prefix =
		env::var("CARGO_FEATURE_UNPREFIXED_MALLOC_ON_SUPPORTED_PLATFORMS").is_err();

	if !use_prefix
		&& NO_UNPREFIXED_MALLOC_TARGETS
			.iter()
			.any(|i| target.contains(i))
	{
		warning!(
			"Unprefixed `malloc` requested on unsupported platform `{}` => using prefixed \
			 `malloc`",
			target
		);
		use_prefix = true;
	}

	// this has to occur before the early return when JEMALLOC_OVERRIDE is set
	println!("cargo::rustc-check-cfg=cfg(prefixed)");
	println!("cargo::rustc-check-cfg=cfg(jevmalloc_docs)");
	if use_prefix {
		println!("cargo:rustc-cfg=prefixed");
	}

	if let Some(jemalloc) = read_and_watch_env_os("JEMALLOC_OVERRIDE") {
		info!("jemalloc override set");
		let jemalloc = PathBuf::from(jemalloc);
		assert!(
			jemalloc.exists(),
			"Path to `jemalloc` in `JEMALLOC_OVERRIDE={}` does not exist",
			jemalloc.display()
		);
		println!("cargo:rustc-link-search=native={}", jemalloc.parent().unwrap().display());
		let stem = jemalloc.file_stem().unwrap().to_str().unwrap();
		let name = jemalloc.file_name().unwrap().to_str().unwrap();
		let kind = if name.ends_with(".a") { "static" } else { "dylib" };
		println!("cargo:rustc-link-lib={}={}", kind, &stem[3..]);
		return;
	}

	let target_arch = rustflags::from_env()
		.find_map(|flag| match flag {
			| RustFlag::Codegen { opt, value } if opt == "target-cpu" => Some(value),
			| _ => None,
		})
		.flatten()
		.or_else(|| {
			env::var("CARGO_CFG_TARGET_ARCH")
				.map(|arch| arch.replacen('_', "-", 1))
				.ok()
		});

	let march: OsString = target_arch
		.map(|arch| format!("-march={arch}"))
		.unwrap_or_default()
		.into();

	let tune_arch = rustflags::from_env().find_map(|flag| match flag {
		| RustFlag::Z(s) if s.starts_with("tune-cpu") =>
			s.split_once('=').map(|(_, v)| v.to_owned()),
		| _ => None,
	});

	let mtune: OsString = tune_arch
		.map(|arch| format!("-mtune={arch}"))
		.unwrap_or_default()
		.into();

	let compiler = cc::Build::new()
		.no_default_flags(true)
		.inherit_rustflags(true)
		.warnings(true)
		.extra_warnings(true)
		.flag(march.as_os_str())
		.flag(mtune.as_os_str())
		.get_compiler();

	let cflags = compiler
		.args()
		.iter()
		.map(|s| s.to_str().unwrap())
		.collect::<Vec<_>>()
		.join(" ");

	info!("CC={:?}", compiler.path());
	info!("CFLAGS={:?}", cflags);

	assert!(out_dir.exists(), "OUT_DIR does not exist");
	let jemalloc_repo_dir = PathBuf::from("jemalloc");
	info!("JEMALLOC_REPO_DIR={:?}", jemalloc_repo_dir);

	if build_dir.exists() {
		fs::remove_dir_all(build_dir.clone()).unwrap();
	}
	// Copy jemalloc submodule to the OUT_DIR
	copy_recursively(&jemalloc_repo_dir, &build_dir)
		.expect("failed to copy jemalloc source code to OUT_DIR");
	assert!(build_dir.exists());

	// Configuration files
	let config_files = ["configure"];

	// Copy the configuration files to jemalloc's source directory
	for f in &config_files {
		fs::copy(Path::new("configure").join(f), build_dir.join(f))
			.expect("failed to copy config file to OUT_DIR");
	}

	// Run configure:
	let configure = build_dir.join("configure");
	let mut cmd = Command::new("sh");
	cmd.arg(
		configure
			.to_str()
			.unwrap()
			.replace("C:\\", "/c/")
			.replace('\\', "/"),
	)
	.current_dir(&build_dir)
	.env("CC", compiler.path())
	.env("EXTRA_CFLAGS", cflags)
	.arg(format!("--with-version={je_version}"))
	.arg("--disable-cxx")
	.arg("--enable-doc=no")
	.arg("--enable-shared=no");

	if target.contains("ios") {
		// newer iOS deviced have 16kb page sizes:
		// closed: https://github.com/gnzlbg/jemallocator/issues/68
		cmd.arg("--with-lg-page=14");
	}

	// collect `malloc_conf` string:
	let mut malloc_conf = String::new();
	if let Ok(malloc_conf_opts) = read_and_watch_env("JEMALLOC_SYS_WITH_MALLOC_CONF") {
		if !malloc_conf.is_empty() {
			malloc_conf.push(',');
		}
		malloc_conf.push_str(&malloc_conf_opts);
	}

	if !malloc_conf.is_empty() {
		info!("--with-malloc-conf={}", malloc_conf);
		cmd.arg(format!("--with-malloc-conf={malloc_conf}"));
	}

	if let Ok(lg_page) = read_and_watch_env("JEMALLOC_SYS_WITH_LG_PAGE") {
		info!("--with-lg-page={}", lg_page);
		cmd.arg(format!("--with-lg-page={lg_page}"));
	}

	if let Ok(lg_hugepage) = read_and_watch_env("JEMALLOC_SYS_WITH_LG_HUGEPAGE") {
		info!("--with-lg-hugepage={}", lg_hugepage);
		cmd.arg(format!("--with-lg-hugepage={lg_hugepage}"));
	}

	if let Ok(lg_quantum) = read_and_watch_env("JEMALLOC_SYS_WITH_LG_QUANTUM") {
		info!("--with-lg-quantum={}", lg_quantum);
		cmd.arg(format!("--with-lg-quantum={lg_quantum}"));
	}

	if let Ok(lg_vaddr) = read_and_watch_env("JEMALLOC_SYS_WITH_LG_VADDR") {
		info!("--with-lg-vaddr={}", lg_vaddr);
		cmd.arg(format!("--with-lg-vaddr={lg_vaddr}"));
	}

	if use_prefix {
		cmd.arg("--with-jemalloc-prefix=_rjem_");
		info!("--with-jemalloc-prefix=_rjem_");
	}

	cmd.arg("--with-private-namespace=_rjem_");

	if cfg!(debug_assertions) || env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok() {
		info!("CARGO_CFG_DEBUG_ASSERTIONS set");
		cmd.arg("--enable-debug");
	} else {
		info!("CARGO_CFG_DEBUG_ASSERTIONS not set");
		cmd.arg("--disable-debug");
		cmd.env("CPPFLAGS", "-DNDEBUG");
	}

	if env::var("CARGO_FEATURE_CACHE_OBLIVIOUS").is_ok() {
		info!("CARGO_FEATURE_CACHE_OBLIVIOUS set");
		cmd.arg("--enable-cache-oblivious");
	} else {
		info!("CARGO_FEATURE_CACHE_OBLIVIOUS not set");
		cmd.arg("--disable-cache-oblivious");
	}

	if env::var("CARGO_FEATURE_CHECK_SAFETY").is_ok() {
		info!("CARGO_FEATURE_CHECK_SAFETY set");
		cmd.arg("--enable-opt-safety-checks");
	} else {
		info!("CARGO_FEATURE_CHECK_SAFETY not set");
		cmd.arg("--disable-opt-safety-checks");
	}

	if env::var("CARGO_FEATURE_CHECK_SIZE_MATCH").is_ok() {
		info!("CARGO_FEATURE_CHECK_SIZE_MATCH set");
		cmd.arg("--enable-opt-size-checks");
	} else {
		info!("CARGO_FEATURE_CHECK_SIZE_MATCH not set");
		cmd.arg("--disable-opt-size-checks");
	}

	if env::var("CARGO_FEATURE_CHECK_USE_AFTER_FREE").is_ok() {
		info!("CARGO_FEATURE_CHECK_USE_AFTER_FREE set");
		cmd.arg("--enable-uaf-detection");
	} else {
		info!("CARGO_FEATURE_CHECK_USE_AFTER_FREE not set");
		cmd.arg("--disable-uaf-detection");
	}

	if env::var("CARGO_FEATURE_FILL").is_ok() {
		info!("CARGO_FEATURE_FILL set");
		cmd.arg("--enable-fill");
	} else {
		info!("CARGO_FEATURE_FILL not set");
		cmd.arg("--disable-fill");
	}

	if env::var("CARGO_FEATURE_INITIAL_EXEC_TLS").is_ok() {
		info!("CARGO_FEATURE_INITIAL_EXEC_TLS set");
		cmd.arg("--enable-initial-exec-tls");
	} else {
		info!("CARGO_FEATURE_INITIAL_EXEC_TLS not set");
		cmd.arg("--disable-initial-exec-tls");
	}

	if env::var("CARGO_FEATURE_PROFILING").is_ok() {
		info!("CARGO_FEATURE_PROFILING set");
		cmd.arg("--enable-prof");
	} else {
		info!("CARGO_FEATURE_PROFILING not set");
		cmd.arg("--disable-prof");
	}

	if env::var("CARGO_FEATURE_STATS").is_ok() {
		info!("CARGO_FEATURE_STATS set");
		cmd.arg("--enable-stats");
	} else {
		info!("CARGO_FEATURE_STATS not set");
		cmd.arg("--disable-stats");
	}

	cmd.arg(format!("--host={}", gnu_target(&target)));
	cmd.arg(format!("--build={}", gnu_target(&host)));
	cmd.arg(format!("--prefix={}", out_dir.display()));

	run_and_log(&mut cmd, &build_dir.join("config.log"));

	// Make:
	let make = make_cmd(&host);
	run(Command::new(make)
		.current_dir(&build_dir)
		.arg("-j")
		.arg(num_jobs.clone()));

	// Skip watching this environment variables to avoid rebuild in CI.
	if env::var("JEMALLOC_SYS_RUN_JEMALLOC_TESTS").is_ok() {
		info!("Building and running jemalloc tests...");
		// Make tests:
		run(Command::new(make)
			.current_dir(&build_dir)
			.arg("-j")
			.arg(num_jobs.clone())
			.arg("tests"));

		// Run tests:
		run(Command::new(make)
			.current_dir(&build_dir)
			.arg("check"));
	}

	// Make install:
	run(Command::new(make)
		.current_dir(&build_dir)
		.arg("install_lib_static")
		.arg("install_include")
		.arg("-j")
		.arg(num_jobs));

	println!("cargo:root={}", out_dir.display());

	// Linkage directives to pull in jemalloc and its dependencies.
	//
	// On some platforms we need to be sure to link in `pthread` which jemalloc
	// depends on, and specifically on android we need to also link to libgcc.
	// Currently jemalloc is compiled with gcc which will generate calls to
	// intrinsics that are libgcc specific (e.g. those intrinsics aren't present in
	// libcompiler-rt), so link that in to get that support.
	if target.contains("windows") {
		println!("cargo:rustc-link-lib=static=jemalloc");
	} else {
		println!("cargo:rustc-link-lib=static=jemalloc_pic");
	}
	println!("cargo:rustc-link-search=native={}/lib", build_dir.display());
	if target.contains("android") {
		println!("cargo:rustc-link-lib=gcc");
	} else if !target.contains("windows") {
		println!("cargo:rustc-link-arg=-pthread");
	}
	// GCC may generate a __atomic_exchange_1 library call which requires -latomic
	// during the final linking. https://github.com/riscv-collab/riscv-gcc/issues/12
	if target.contains("riscv") {
		println!("cargo:rustc-link-lib=atomic");
	}
	println!("cargo:rerun-if-changed=jemalloc");

	if target.contains("android") {
		// These symbols are used by jemalloc on android but the really old android
		// we're building on doesn't have them defined, so just make sure the symbols
		// are available.
		cc::Build::new()
			.file("src/pthread_atfork.c")
			.compile("pthread_atfork");
		println!("cargo:rerun-if-changed=src/pthread_atfork.c");
	}
}

fn run_and_log(cmd: &mut Command, log_file: &Path) {
	execute(cmd, || {
		run(Command::new("tail")
			.arg("-n")
			.arg("100")
			.arg(log_file));
	});
}

fn run(cmd: &mut Command) { execute(cmd, || ()); }

fn execute(cmd: &mut Command, on_fail: impl FnOnce()) {
	println!("running: {cmd:?}");
	let status = match cmd.status() {
		| Ok(status) => status,
		| Err(e) => panic!("failed to execute command: {e}"),
	};
	if !status.success() {
		on_fail();
		panic!("command did not execute successfully: {cmd:?}\nexpected success, got: {status}");
	}
}

fn gnu_target(target: &str) -> String {
	match target {
		| "i686-pc-windows-msvc" => "i686-pc-win32".to_owned(),
		| "x86_64-pc-windows-msvc" => "x86_64-pc-win32".to_owned(),
		| "i686-pc-windows-gnu" => "i686-w64-mingw32".to_owned(),
		| "x86_64-pc-windows-gnu" => "x86_64-w64-mingw32".to_owned(),
		| "armv7-linux-androideabi" => "arm-linux-androideabi".to_owned(),
		| "riscv64gc-unknown-linux-gnu" => "riscv64-linux-gnu".to_owned(),
		| "riscv64gc-unknown-linux-musl" => "riscv64-linux-musl".to_owned(),
		| s => s.to_owned(),
	}
}

fn make_cmd(host: &str) -> &'static str {
	const GMAKE_HOSTS: &[&str] =
		&["bitrig", "dragonfly", "freebsd", "netbsd", "openbsd", "chimera-linux"];
	if GMAKE_HOSTS.iter().any(|i| host.contains(i)) {
		"gmake"
	} else if host.contains("windows") {
		"mingw32-make"
	} else {
		"make"
	}
}

fn read_and_watch_env_impl<T, F>(name: &str, env_getter: F) -> Option<T>
where
	F: Fn(&str) -> Option<T>,
{
	let prefix = env::var("TARGET")
		.unwrap()
		.to_uppercase()
		.replace('-', "_");

	let prefixed_name = format!("{prefix}_{name}");

	println!("cargo:rerun-if-env-changed={prefixed_name}");
	if let Some(value) = env_getter(&prefixed_name) {
		return Some(value);
	}

	println!("cargo:rerun-if-env-changed={name}");
	env_getter(name)
}

fn read_and_watch_env(name: &str) -> Result<String, env::VarError> {
	read_and_watch_env_impl(name, |n| env::var(n).ok()).ok_or(env::VarError::NotPresent)
}

fn read_and_watch_env_os(name: &str) -> Option<OsString> {
	read_and_watch_env_impl(name, |n| env::var_os(n))
}

fn copy_recursively(src: &Path, dst: &Path) -> io::Result<()> {
	if !dst.exists() {
		fs::create_dir_all(dst)?;
	}
	for entry in fs::read_dir(src)? {
		let entry = entry?;
		let ft = entry.file_type()?;
		if ft.is_dir() {
			// There should be very few layer in the project, use recusion to keep simple.
			copy_recursively(&entry.path(), &dst.join(entry.file_name()))?;
		} else {
			fs::copy(entry.path(), dst.join(entry.file_name()))?;
		}
	}
	Ok(())
}

fn expect_env(name: &str) -> String {
	env::var(name).unwrap_or_else(|_| panic!("{name} was not set"))
}

//! The `MALLCTL` namespace, statistics, and the compile-time configuration.
//!
//! `mallctl` addresses a tree-structured namespace by period-separated name,
//! which `mallctlnametomib` translates once into an integer path for callers
//! reading the same node repeatedly. The two statics reach the same surface
//! from the other side: `malloc_conf` seeds the `opt.*` subtree before `main`
//! runs, and `malloc_message` is where the allocator's own output goes.

use libc::{c_char, c_int, c_void, size_t};

unsafe extern "C" {
	/// Compile-time string of configuration options.
	///
	/// Once, when the first call is made to one of the memory allocation
	/// routines, the allocator initializes its internals based in part on
	/// various options that can be specified at compile- or run-time.
	///
	/// The string specified via `--with-malloc-conf`, the string pointed to by
	/// the global variable `malloc_conf`, the “name” of the file referenced by
	/// the symbolic link named `/etc/malloc.conf`, and the value of the
	/// environment variable `MALLOC_CONF`, will be interpreted, in that order,
	/// from left to right as options. Note that `malloc_conf` may be read
	/// before `main()` is entered, so the declaration of `malloc_conf` should
	/// specify an initializer that contains the final value to be read by
	/// `jemalloc`.
	///
	/// `--with-malloc-conf` and `malloc_conf` are compile-time mechanisms,
	/// whereas `/etc/malloc.conf` and `MALLOC_CONF` can be safely set any time
	/// prior to program invocation.
	///
	/// An options string is a comma-separated list of `option:value` pairs.
	/// There is one key corresponding to each `opt.* mallctl` (see the `MALLCTL
	/// NAMESPACE` section for options documentation). For example,
	/// `abort:true,narenas:1` sets the `opt.abort` and `opt.narenas` options.
	/// Some options have boolean values (`true`/`false`), others have integer
	/// values (base `8`, `10`, or `16`, depending on prefix), and yet others
	/// have raw string values.
	#[cfg_attr(prefixed, link_name = "_rjem_malloc_conf")]
	pub static malloc_conf: Option<&'static c_char>;

	/// Allows overriding the function which emits the text strings forming the
	/// errors and warnings if for some reason the `STDERR_FILENO` file
	/// descriptor is not suitable for this.
	///
	/// [`malloc_message`] takes the `cbopaque` pointer argument that is null,
	/// unless overridden by the arguments in a call to [`malloc_stats_print`],
	/// followed by a string pointer.
	///
	/// Please note that doing anything which tries to allocate memory in this
	/// function is likely to result in a crash or deadlock.
	#[cfg_attr(prefixed, link_name = "_rjem_malloc_message")]
	pub static mut malloc_message:
		Option<unsafe extern "C" fn(cbopaque: *mut c_void, s: *const c_char)>;

	/// General interface for introspecting the memory allocator, as well as
	/// setting modifiable parameters and triggering actions.
	///
	/// The period-separated name argument specifies a location in a
	/// tree-structured namespace ([see jemalloc's `MALLCTL`
	/// documentation][jemalloc_mallctl]).
	///
	/// To read a value, pass a pointer via `oldp` to adequate space to contain
	/// the value, and a pointer to its length via `oldlenp`; otherwise pass
	/// null and null. Similarly, to write a value, pass a pointer to the value
	/// via `newp`, and its length via `newlen`; otherwise pass null and 0.
	///
	/// # Errors
	///
	/// Returns `0` on success, otherwise returns:
	///
	/// * `EINVAL`: if `newp` is not null, and `newlen` is too large or too
	///   small. Alternatively, `*oldlenp` is too large or too small; in this
	///   case as much data as possible are read despite the error.
	///
	/// * `ENOENT`: `name` or mib specifies an unknown/invalid value.
	///
	/// * `EPERM`: Attempt to read or write void value, or attempt to write
	///   read-only value.
	///
	/// * `EAGAIN`: A memory allocation failure occurred.
	///
	/// * `EFAULT`: An interface with side effects failed in some way not
	///   directly related to `mallctl` read/write processing.
	///
	/// [jemalloc_mallctl]: http://jemalloc.net/jemalloc.3.html#mallctl_namespace
	#[cfg_attr(prefixed, link_name = "_rjem_mallctl")]
	pub fn mallctl(
		name: *const c_char,
		oldp: *mut c_void,
		oldlenp: *mut size_t,
		newp: *mut c_void,
		newlen: size_t,
	) -> c_int;

	/// Translates a name to a “Management Information Base” (MIB) that can be
	/// passed repeatedly to [`mallctlbymib`].
	///
	/// This avoids repeated name lookups for applications that repeatedly query
	/// the same portion of the namespace.
	///
	/// On success, `mibp` contains an array of `*miblenp` integers, where
	/// `*miblenp` is the lesser of the number of components in name and the
	/// input value of `*miblenp`. Thus it is possible to pass a `*miblenp` that
	/// is smaller than the number of period-separated name components, which
	/// results in a partial MIB that can be used as the basis for constructing
	/// a complete MIB. For name components that are integers (e.g. the 2 in
	/// arenas.bin.2.size), the corresponding MIB component will always be that
	/// integer.
	#[cfg_attr(prefixed, link_name = "_rjem_mallctlnametomib")]
	pub fn mallctlnametomib(
		name: *const c_char,
		mibp: *mut size_t,
		miblenp: *mut size_t,
	) -> c_int;

	/// Like [`mallctl`] but taking a `mib` as input instead of a name.
	#[cfg_attr(prefixed, link_name = "_rjem_mallctlbymib")]
	pub fn mallctlbymib(
		mib: *const size_t,
		miblen: size_t,
		oldp: *mut c_void,
		oldlenp: *mut size_t,
		newp: *mut c_void,
		newlen: size_t,
	) -> c_int;

	/// Writes summary statistics via the `write_cb` callback function pointer
	/// and `cbopaque` data passed to `write_cb`, or [`malloc_message`] if
	/// `write_cb` is null.
	///
	/// The statistics are presented in human-readable form unless “J”
	/// is specified as a character within the opts string, in which case the
	/// statistics are presented in JSON format.
	///
	/// This function can be called repeatedly.
	///
	/// General information that never changes during execution can be omitted
	/// by specifying `g` as a character within the opts string.
	///
	/// Note that [`malloc_message`] uses the `mallctl*` functions internally,
	/// so inconsistent statistics can be reported if multiple threads use these
	/// functions simultaneously.
	///
	/// If the Cargo feature `stats` is enabled, `m`, `d`, and `a` can be
	/// specified to omit merged arena, destroyed merged arena, and per arena
	/// statistics, respectively; `b` and `l` can be specified to omit per size
	/// class statistics for bins and large objects, respectively; `x` can be
	/// specified to omit all mutex statistics. Unrecognized characters are
	/// silently ignored.
	///
	/// Note that thread caching may prevent some statistics from being
	/// completely up to date, since extra locking would be required to merge
	/// counters that track thread cache operations.
	#[cfg_attr(prefixed, link_name = "_rjem_malloc_stats_print")]
	pub fn malloc_stats_print(
		write_cb: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
		cbopaque: *mut c_void,
		opts: *const c_char,
	);
}

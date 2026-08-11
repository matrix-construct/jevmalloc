//! Fixed-capacity arena names.

use core::{
	ffi::CStr,
	fmt, result,
	str::{self, Utf8Error},
};

use libc::c_char;

/// Number of bytes in jemalloc's arena name storage, including its terminator.
pub const ARENA_NAME_LEN: usize = 32;

/// An arena name copied out of jemalloc's internal storage.
///
/// The inline buffer contains at most 31 content bytes followed by a NUL. Names
/// are byte strings and need not contain valid UTF-8.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ArenaName([c_char; ARENA_NAME_LEN]);

impl ArenaName {
	/// Returns the name as a terminated C string.
	///
	/// The returned string borrows this inline buffer and performs no
	/// allocation or encoding conversion.
	#[must_use]
	pub fn as_c_str(&self) -> &CStr {
		// SAFETY: construction initializes the buffer with zeros, and jemalloc's
		// name getter preserves a terminator in its fixed-size destination.
		unsafe { CStr::from_ptr(self.0.as_ptr()) }
	}

	/// Returns the content bytes without the trailing terminator.
	///
	/// The bytes preserve the name exactly as returned by jemalloc.
	#[must_use]
	pub fn as_bytes(&self) -> &[u8] { self.as_c_str().to_bytes() }

	/// Returns the name as UTF-8 when its bytes are valid.
	///
	/// # Errors
	///
	/// Returns the standard UTF-8 error when the name contains invalid bytes.
	pub fn to_str(&self) -> result::Result<&str, Utf8Error> { str::from_utf8(self.as_bytes()) }

	/// Returns the writable destination used by the arena name control.
	pub(super) fn as_mut_ptr(&mut self) -> *mut c_char { self.0.as_mut_ptr() }
}

impl AsRef<CStr> for ArenaName {
	fn as_ref(&self) -> &CStr { self.as_c_str() }
}

impl Default for ArenaName {
	fn default() -> Self { Self([0; ARENA_NAME_LEN]) }
}

impl fmt::Debug for ArenaName {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_tuple("ArenaName")
			.field(&self.as_c_str())
			.finish()
	}
}

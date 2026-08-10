//! Error reporting for jemalloc control operations.
//!
//! The control API returns integer status codes. Each nonzero status is
//! preserved as an [`Error`] with descriptions for the codes documented by
//! jemalloc.

use libc::c_int;

use super::Result;
use crate::std::{error, fmt, num};

/// Selects a nonzero unsigned representation with the width of a C integer.
pub(super) trait NonZeroT {
	/// The corresponding nonzero unsigned integer type.
	type T;
}
impl NonZeroT for i32 {
	type T = num::NonZeroU32;
}
impl NonZeroT for i64 {
	type T = num::NonZeroU64;
}

/// Nonzero storage for an error code returned as [`c_int`].
pub(super) type NonZeroCInt = <c_int as NonZeroT>::T;

/// A nonzero status returned by a jemalloc control function.
///
/// The `mallctl`, `mallctlnametomib`, and `mallctlbymib` functions return zero
/// on success. This type retains any other return value and formats the known
/// jemalloc error codes with a descriptive message.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Error(
	/// The nonzero error code returned by jemalloc.
	NonZeroCInt,
);

impl fmt::Debug for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let code = self.0.get() as c_int;
		match description(code) {
			| Some(m) => write!(f, "{m}"),
			| None => write!(f, "Unknown error code: \"{code}\"."),
		}
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		<Self as fmt::Debug>::fmt(self, f)
	}
}

impl error::Error for Error {}

/// Returns jemalloc's description for a recognized error code.
fn description(code: c_int) -> Option<&'static str> {
	match code {
		| libc::EINVAL => Some(
			"`newp` is not `NULL`, and `newlen` is too large or too small. Alternatively, \
			 `*oldlenp` is too large or too small; in this case as much data as possible are \
			 read despite the error.",
		),
		| libc::ENOENT => Some("`name` or `mib` specifies an unknown/invalid value."),
		| libc::EPERM =>
			Some("Attempt to read or write `void` value, or attempt to write read-only value."),
		| libc::EAGAIN => Some("A memory allocation failure occurred."),
		| libc::EFAULT => Some(
			"An interface with side effects failed in some way not directly related to \
			 `mallctl*()` read/write processing.",
		),
		| _ => None,
	}
}

/// Converts a jemalloc status code into a control result.
///
/// # Errors
///
/// Returns [`Error`] when `ret` is nonzero.
pub(crate) fn cvt(ret: c_int) -> Result<()> {
	match ret {
		| 0 => Ok(()),
		| v => Err(Error(unsafe { NonZeroCInt::new_unchecked(v as u32) })),
	}
}

#[cfg(test)]
mod tests {
	//! Checks the compact representation of control errors.

	use super::*;

	/// Confirms that the nonzero code preserves `Result` niche optimization.
	#[test]
	fn size_of_result_error() {
		use core::mem::size_of;
		assert_eq!(size_of::<Result<()>>(), size_of::<Error>());
		assert_eq!(size_of::<Error>(), size_of::<c_int>());
	}
}

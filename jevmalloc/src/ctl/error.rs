//! Error reporting for jemalloc control operations.

use libc::c_int;

use super::Result;
use crate::std::{error, fmt, num::NonZeroI32};

/// A nonzero status returned by jemalloc's control interface.
///
/// The status is an errno value such as `ENOENT` or `EINVAL`. Wrapper-side
/// validation failures use `EINVAL`, so callers can handle invalid ad hoc
/// control names without a panic.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Error(NonZeroI32);

impl Error {
	/// Returns the numeric errno value.
	#[must_use]
	pub const fn code(self) -> c_int { self.0.get() }

	/// Reports whether this error has the supplied errno value.
	#[must_use]
	pub const fn is(self, code: c_int) -> bool { self.code() == code }

	/// Constructs an error from a status known to be nonzero.
	pub(super) fn from_code(code: c_int) -> Self {
		Self(NonZeroI32::new(code).expect("jemalloc error status must be nonzero"))
	}

	/// Constructs the wrapper's invalid-argument error.
	pub(super) fn invalid_argument() -> Self { Self::from_code(libc::EINVAL) }

	/// Constructs the wrapper's invalid-pointer error.
	pub(super) fn bad_address() -> Self { Self::from_code(libc::EFAULT) }

	/// Returns the standard description for a recognized status.
	fn description(self) -> Option<&'static str> {
		use libc::{EAGAIN, EFAULT, EINVAL, ENOENT, EPERM};

		match self.code() {
			| EAGAIN => Some("Resource temporarily unavailable"),
			| EFAULT => Some("Bad address"),
			| EINVAL => Some("Invalid argument"),
			| ENOENT => Some("No such entity"),
			| EPERM => Some("Operation not permitted"),
			| _ => None,
		}
	}
}

impl error::Error for Error {}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.description() {
			| Some(description) => write!(f, "{description} (errno {})", self.code()),
			| None => write!(f, "Unknown jemalloc error (errno {})", self.code()),
		}
	}
}

impl fmt::Debug for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(self, f) }
}

/// Converts a jemalloc return status into a control result.
pub(super) fn cvt(code: c_int) -> Result {
	match code {
		| 0 => Ok(()),
		| code => Err(Error::from_code(code)),
	}
}

#[cfg(test)]
mod tests {
	//! Checks error representation and formatting.

	extern crate std as rust_std;

	use rust_std::string::ToString;

	use super::*;

	/// Confirms that the nonzero code preserves the `Result` niche.
	#[test]
	fn result_uses_the_error_niche() {
		use core::mem::size_of;

		assert_eq!(size_of::<Result<()>>(), size_of::<Error>());
		assert_eq!(size_of::<Error>(), size_of::<c_int>());
	}

	/// Confirms that unknown statuses retain their numeric value in
	/// diagnostics.
	#[test]
	fn unknown_status_keeps_its_code() {
		let error = Error::from_code(12345);

		assert_eq!(error.code(), 12345);
		assert!(error.to_string().contains("12345"));
	}
}

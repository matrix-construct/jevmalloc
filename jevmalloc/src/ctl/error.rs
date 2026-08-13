//! Error reporting for jemalloc control operations.

use core::{error, fmt, num::NonZeroI32};

use libc::c_int;

/// A nonzero errno-style allocator operation error.
///
/// Statuses returned by jemalloc retain their errno values. Wrapper-side
/// failures use the same representation, so callers can distinguish invalid
/// input, insufficient storage, and invalid text without a panic.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Error(NonZeroI32);

impl Error {
	/// Returns the numeric errno value.
	#[must_use]
	#[inline]
	pub const fn code(self) -> c_int { self.0.get() }

	/// Reports whether this error has the supplied errno value.
	#[must_use]
	#[inline]
	pub const fn is(self, code: c_int) -> bool { self.code() == code }

	/// Constructs the wrapper's invalid-argument error.
	#[inline]
	pub(crate) fn invalid_argument() -> Self { Self::from_code(libc::EINVAL) }

	/// Constructs the wrapper's invalid-pointer error.
	#[inline]
	pub(crate) fn bad_address() -> Self { Self::from_code(libc::EFAULT) }

	/// Constructs the wrapper's insufficient-space error.
	#[inline]
	pub(crate) fn insufficient_space() -> Self { Self::from_code(libc::ENOSPC) }

	/// Constructs the wrapper's invalid-UTF-8 error.
	#[inline]
	pub(crate) fn invalid_utf8() -> Self { Self::from_code(libc::EILSEQ) }

	/// Constructs an error from a status known to be nonzero.
	#[inline]
	pub(super) fn from_code(code: c_int) -> Self {
		Self(NonZeroI32::new(code).expect("jemalloc error status must be nonzero"))
	}

	/// Returns the standard description for a recognized status.
	fn description(self) -> Option<&'static str> {
		use libc::{EAGAIN, EFAULT, EILSEQ, EINVAL, ENOENT, ENOSPC, EPERM};

		match self.code() {
			| EAGAIN => Some("Resource temporarily unavailable"),
			| EFAULT => Some("Bad address"),
			| EILSEQ => Some("Invalid byte sequence"),
			| EINVAL => Some("Invalid argument"),
			| ENOENT => Some("No such entity"),
			| ENOSPC => Some("Insufficient space"),
			| EPERM => Some("Operation not permitted"),
			| _ => None,
		}
	}
}

impl error::Error for Error {}

impl fmt::Debug for Error {
	#[inline]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(self, f) }
}

impl fmt::Display for Error {
	#[inline(never)]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.description() {
			| Some(description) => write!(f, "{description} (errno {})", self.code()),
			| None => write!(f, "Unknown jemalloc error (errno {})", self.code()),
		}
	}
}

#[cfg(test)]
mod tests {
	//! Checks error representation and formatting.

	extern crate std as rust_std;

	use rust_std::string::ToString;

	use super::{super::Result, *};

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

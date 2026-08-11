//! Typed dynamic storage allocation precedence.

use core::ffi::CStr;

use libc::c_char;

use crate::ctl::{Error, Result};

/// Dynamic storage allocation precedence for one arena or the future-arena
/// default.
///
/// Platforms without `sbrk(2)` support only [`Dss::Disabled`]. A custom extent
/// allocation hook makes this setting irrelevant for that arena. Retained DSS
/// mappings leak when a destructible arena is destroyed, so such arenas should
/// normally disable DSS or use hooks that track those mappings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dss {
	/// Never use dynamic storage allocation.
	Disabled,

	/// Try dynamic storage allocation before memory mapping.
	Primary,

	/// Try dynamic storage allocation after memory mapping.
	Secondary,
}

impl Dss {
	/// Returns the static C string accepted by jemalloc.
	pub(crate) const fn as_ptr(self) -> *const c_char {
		match self {
			| Self::Disabled => c"disabled".as_ptr(),
			| Self::Primary => c"primary".as_ptr(),
			| Self::Secondary => c"secondary".as_ptr(),
		}
	}

	/// Converts one of jemalloc's static precedence strings.
	///
	/// # Errors
	///
	/// Returns an error for a null pointer or an undocumented value.
	///
	/// # Safety
	///
	/// `ptr` must address a readable, NUL-terminated C string for the duration
	/// of this call.
	pub(crate) unsafe fn from_ptr(ptr: *const c_char) -> Result<Self> {
		if ptr.is_null() {
			return Err(Error::bad_address());
		}

		// SAFETY: the caller guarantees a readable, terminated C string.
		let value = unsafe { CStr::from_ptr(ptr) }.to_bytes();
		match value {
			| b"disabled" => Ok(Self::Disabled),
			| b"primary" => Ok(Self::Primary),
			| b"secondary" => Ok(Self::Secondary),
			| _ => Err(Error::invalid_argument()),
		}
	}
}

//! Portable representations for typed control values.
//!
//! The MSVC representation follows cl.exe. Clang-cl uses a one-byte `_Bool`
//! and remains outside the crate's supported target matrix.

#[cfg(target_env = "msvc")]
use libc::c_int;

use super::{Key, Result, raw};

/// Jemalloc's C `bool` representation when built by cl.exe.
#[cfg(target_env = "msvc")]
type CBool = c_int;

/// Jemalloc's C `_Bool` representation on other targets.
#[cfg(not(target_env = "msvc"))]
type CBool = bool;

/// Reads a control whose C output type is `bool`.
pub(crate) fn get_bool(key: &Key) -> Result<bool> {
	// SAFETY: callers select a control with the platform's `CBool` output type.
	unsafe { raw::get::<CBool>(key) }.map(decode_bool)
}

/// Exchanges a control whose C input and output types are both `bool`.
pub(crate) fn xchg_bool(key: &Key, value: bool) -> Result<bool> {
	let value = encode_bool(value);

	// SAFETY: callers select a control with `CBool` input and output types.
	unsafe { raw::xchg(key, &value) }.map(decode_bool)
}

/// Converts a Rust boolean to jemalloc's platform C representation.
#[cfg(target_env = "msvc")]
fn encode_bool(value: bool) -> CBool { if value { 1 } else { 0 } }

/// Preserves the native C `_Bool` representation.
#[cfg(not(target_env = "msvc"))]
fn encode_bool(value: bool) -> CBool { value }

/// Converts jemalloc's platform C representation to a Rust boolean.
#[cfg(target_env = "msvc")]
fn decode_bool(value: CBool) -> bool { value != 0 }

/// Preserves the native C `_Bool` representation.
#[cfg(not(target_env = "msvc"))]
fn decode_bool(value: CBool) -> bool { value }

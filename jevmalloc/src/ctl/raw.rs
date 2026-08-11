//! Low-level MIB access to jemalloc controls.
//!
//! [`mibs`] safely resolves a control name. The generic operations are unsafe
//! because jemalloc's numeric MIB encodes neither its C value type nor the
//! semantic preconditions and effects of a command.
//! Prefer the typed functions in the parent module whenever one exists.

use libc::{c_char, c_int, c_void, size_t};

use super::{Error, KEY_SEGS, Key, NAME_MAX, Result};
use crate::{
	ffi,
	std::{
		mem::{MaybeUninit, size_of},
		ptr::{addr_of, addr_of_mut, null_mut},
	},
};

/// Resolves a jemalloc control name into a reusable MIB key.
///
/// The input omits the trailing NUL. Names with an interior NUL, names that do
/// not fit the fixed stack buffer, and names deeper than [`Key`]'s inline
/// capacity are rejected as `EINVAL`.
///
/// # Errors
///
/// Returns an error if the name is invalid or jemalloc does not recognize it.
#[inline]
pub fn mibs(name: &str) -> Result<Key> {
	let bytes = name.as_bytes();
	let segments = name.split('.').count();

	if bytes.is_empty() || bytes.len() >= NAME_MAX || bytes.contains(&0) || segments > KEY_SEGS {
		return Err(Error::invalid_argument());
	}

	let mut name_buf = [0_u8; NAME_MAX];
	name_buf[..bytes.len()].copy_from_slice(bytes);

	let mut key = Key::new();
	let mut key_len = key.capacity();
	let key_ptr = key
		.spare_capacity_mut()
		.as_mut_ptr()
		.cast::<size_t>();

	// SAFETY: the validated name leaves a trailing NUL in `name_buf`.
	// `key_ptr` has `key_len` aligned, writable `size_t` slots, and every
	// pointee remains live for the call.
	let status = unsafe {
		ffi::mallctlnametomib(
			name_buf.as_ptr().cast::<c_char>(),
			key_ptr,
			addr_of_mut!(key_len).cast::<size_t>(),
		)
	};

	into_result(status)?;

	if key_len != segments || key_len > key.capacity() {
		return Err(Error::invalid_argument());
	}

	// SAFETY: successful translation initialized the first `key_len` slots,
	// and the checks above establish that they are within capacity.
	unsafe { key.set_len(key_len) };

	Ok(key)
}

/// Reads the value selected by `key` as `T`.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the MIB, access, or output size.
///
/// # Panics
///
/// Panics if `T` is zero-sized or jemalloc succeeds with an output size other
/// than `size_of::<T>()`.
///
/// # Safety
///
/// `key` must be the exact, complete MIB for a readable control. `T` must
/// exactly match that control's C output type, including its size, alignment,
/// and validity requirements. In particular, reading a C byte as `bool` is
/// valid only when the control guarantees a zero or one representation.
///
/// `arena.<i>.name` is not a conventional pointer-valued read. Jemalloc writes
/// through the pointer already stored in the output slot, which must reference
/// a caller-owned buffer of `ARENA_NAME_LEN` bytes. Do not read that control
/// through this function.
pub unsafe fn get<T: Copy>(key: &Key) -> Result<T> {
	let expected = size_of::<T>();
	let mut value_len = expected;
	let mut value = MaybeUninit::<T>::uninit();

	debug_assert!(expected > 0, "use notify for a no-value control operation");

	// SAFETY: the caller supplies an exact readable MIB and the control's valid
	// C output representation as `T`. The key, output, and length storage are
	// aligned and remain live for the call.
	let status = unsafe {
		ffi::mallctlbymib(
			key.as_ptr(),
			key.len(),
			value.as_mut_ptr().cast::<c_void>(),
			addr_of_mut!(value_len).cast::<size_t>(),
			null_mut(),
			0,
		)
	};

	into_result(status)?;

	assert_eq!(value_len, expected, "jemalloc returned an unexpected control value size");

	// SAFETY: successful completion and the caller's validity guarantee mean
	// `value` contains a valid `T`; its reported size was checked above.
	Ok(unsafe { value.assume_init() })
}

/// Writes `value` to the control selected by `key`.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the MIB, access, or input size.
///
/// # Panics
///
/// Panics if `T` is zero-sized. Use [`notify`] for a no-value operation.
///
/// # Safety
///
/// `key` must be the exact, complete MIB for a writable control. `T` must
/// exactly match that control's C input type. Pointer inputs must satisfy every
/// control-specific pointee, aliasing, and lifetime rule, including any period
/// for which jemalloc retains the pointer.
pub unsafe fn set<T>(key: &Key, value: &T) -> Result {
	let value_len = size_of::<T>();

	debug_assert!(value_len > 0, "use notify for a no-value control operation");

	// SAFETY: the caller supplies an exact writable MIB and its exact C input
	// type. `value` is aligned, readable, and live for the call, and jemalloc
	// treats this shared input slot as read-only. Any contained pointer satisfies
	// the control-specific pointee and retention obligations.
	let status = unsafe {
		ffi::mallctlbymib(
			key.as_ptr(),
			key.len(),
			null_mut(),
			null_mut(),
			addr_of!(*value).cast::<c_void>().cast_mut(),
			value_len,
		)
	};

	into_result(status)
}

/// Supplies a new value to `key` and returns the control-defined output.
///
/// Most read-write controls return the previous value. Some controls define
/// different behavior; an `epoch` refresh returns the newly advanced epoch.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the MIB, access, or value size. Some
/// controls perform their write before detecting an invalid output buffer, so
/// an error does not universally prove that no side effect occurred.
///
/// # Panics
///
/// Panics if `T` is zero-sized or jemalloc succeeds with an output size other
/// than `size_of::<T>()`.
///
/// # Safety
///
/// `key` must be the exact, complete MIB for a control that accepts
/// simultaneous input and output. `T` must exactly match both C types. The
/// input and every successful output must be valid `T` values. Pointer values
/// must satisfy all control-specific accessibility, aliasing, and lifetime
/// requirements.
pub unsafe fn xchg<T: Copy>(key: &Key, value: &T) -> Result<T> {
	let expected = size_of::<T>();
	let mut output_len = expected;
	let mut output = MaybeUninit::<T>::uninit();

	debug_assert!(expected > 0, "use notify for a no-value control operation");

	// SAFETY: the caller supplies an exact read-write MIB and its valid C type.
	// The disjoint storage is aligned and live for the call, and jemalloc treats
	// the shared input slot as read-only. Any contained pointer satisfies the
	// control-specific pointee and retention obligations.
	let status = unsafe {
		ffi::mallctlbymib(
			key.as_ptr(),
			key.len(),
			output.as_mut_ptr().cast::<c_void>(),
			addr_of_mut!(output_len).cast::<size_t>(),
			addr_of!(*value).cast::<c_void>().cast_mut(),
			expected,
		)
	};

	into_result(status)?;

	assert_eq!(output_len, expected, "jemalloc returned an unexpected control value size");

	// SAFETY: successful completion and the caller's validity guarantee mean
	// `output` contains a valid `T`; its reported size was checked above.
	Ok(unsafe { output.assume_init() })
}

/// Invokes a control without input or output values.
///
/// All value pointers and lengths are null or zero. This is also how optional
/// inputs are omitted for controls such as `prof.dump` and `prof.reset`.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the MIB or operation.
///
/// # Safety
///
/// `key` must be the exact, complete MIB for a command that permits every value
/// pointer to be null and every length to be zero. The caller must uphold its
/// semantic preconditions and account for every side effect. Some commands can
/// destroy arenas or invalidate allocator state that safe Rust still relies on.
#[inline]
pub unsafe fn notify(key: &Key) -> Result {
	// SAFETY: the caller supplies an exact no-value command whose contract
	// permits null value pointers and zero lengths. The key remains live.
	let status = unsafe {
		ffi::mallctlbymib(key.as_ptr(), key.len(), null_mut(), null_mut(), null_mut(), 0)
	};

	into_result(status)
}

/// Converts a jemalloc return status into a control result.
#[inline]
fn into_result(code: c_int) -> Result {
	match code {
		| 0 => Ok(()),
		| code => Err(Error::from_code(code)),
	}
}

#[cfg(test)]
mod tests {
	//! Checks name validation before the FFI boundary.

	use super::*;

	/// Rejects an empty name before calling jemalloc.
	#[test]
	fn rejects_empty_name() {
		let error = mibs("").unwrap_err();

		assert!(error.is(libc::EINVAL));
	}

	/// Rejects names that C would truncate at an interior NUL.
	#[test]
	fn rejects_interior_nul() {
		let error = mibs("epoch\0background_thread").unwrap_err();

		assert!(error.is(libc::EINVAL));
	}

	/// Rejects names deeper than the inline MIB representation.
	#[test]
	fn rejects_excessive_depth() {
		let error = mibs("a.b.c.d.e.f.g.h.i").unwrap_err();

		assert!(error.is(libc::EINVAL));
	}

	/// Accepts the longest buffer input and rejects one byte beyond it.
	#[test]
	fn enforces_name_limit() {
		let bytes = [b'a'; NAME_MAX];
		let longest = core::str::from_utf8(&bytes[..NAME_MAX - 1]).unwrap();
		let overlong = core::str::from_utf8(&bytes).unwrap();

		assert!(mibs(longest).unwrap_err().is(libc::ENOENT));
		assert!(mibs(overlong).unwrap_err().is(libc::EINVAL));
	}
}

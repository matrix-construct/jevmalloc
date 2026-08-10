//! Low-level typed access to jemalloc's `mallctl` API.
//!
//! Named and MIB controls are exposed without validating the selected Rust
//! type. Prefer [`super::Name`] and [`super::Mib`] when their safe access
//! traits cover the control.

use libc::c_char;

use super::{Result, error::cvt};
use crate::std::{ptr, slice};

/// Translates a null-terminated control `name` into a MIB.
///
/// A Management Information Base avoids repeated name lookups when callers
/// access the same part of jemalloc's control namespace.
///
/// A destination shorter than the number of period-separated components
/// receives a partial MIB that can be completed by the caller. Numeric name
/// components retain their numeric value in the corresponding MIB component.
///
/// ```
/// #[global_allocator]
/// static ALLOC: jevmalloc::Jemalloc = jevmalloc::Jemalloc;
///
/// fn main() {
/// 	use jevmalloc::ctl::raw;
/// 	use libc::c_uint;
/// 	unsafe {
/// 		let mut mib = [0; 4];
/// 		let nbins: c_uint = raw::read(b"arenas.nbins\0").unwrap();
/// 		raw::name_to_mib(b"arenas.bin.0.size\0", &mut mib).unwrap();
/// 		for i in 0..usize::try_from(nbins).unwrap() {
/// 			mib[2] = i;
/// 			let bin_size: usize = raw::read_mib(&mib).unwrap();
/// 			println!("arena bin {} has size {}", i, bin_size);
/// 		}
/// 	}
/// }
/// ```
///
/// # Errors
///
/// Returns an error when jemalloc rejects or cannot translate `name`.
///
/// # Panics
///
/// Panics if `name` is empty or does not end in NUL, or if jemalloc reports a
/// successful translation shorter than `mib`.
pub fn name_to_mib(name: &[u8], mib: &mut [usize]) -> Result<()> {
	unsafe {
		validate_name(name);

		let mut len = mib.len();
		cvt(jevmalloc_sys::mallctlnametomib(
			ptr::from_ref(name).cast::<c_char>(),
			mib.as_mut_ptr(),
			&raw mut len,
		))?;
		assert_eq!(mib.len(), len);
		Ok(())
	}
}

/// Reads the value addressed by `mib` as `T`.
///
/// Use [`name_to_mib`] to translate a control name when the same key will be
/// read repeatedly.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the MIB, access, or output size.
///
/// # Panics
///
/// Panics if jemalloc succeeds without reporting exactly `size_of::<T>()`
/// output bytes.
///
/// # Safety
///
/// `T` must be layout-compatible with the selected control's output type, and
/// every successful output must be a valid `T`. A same-sized type with stricter
/// validity requirements, such as `u8` read as `bool`, can cause undefined
/// behavior.
///
/// `arena.<i>.name` instead treats the read slot as a pointer to a
/// caller-allocated buffer of at least 32 bytes (`ARENA_NAME_LEN`). Reading
/// that control as `*const c_char` lets jemalloc write through an uninitialized
/// pointer.
pub unsafe fn read_mib<T: Copy>(mib: &[usize]) -> Result<T> {
	unsafe {
		let mut len = size_of::<T>();
		let mut value = MaybeUninit { init: () };
		let value_ptr = (&raw mut value.init).cast();

		cvt(jevmalloc_sys::mallctlbymib(
			mib.as_ptr(),
			mib.len(),
			value_ptr,
			&raw mut len,
			ptr::null_mut(),
			0,
		))?;
		assert_eq!(len, size_of::<T>());
		Ok(value.maybe_uninit)
	}
}

/// Reads the value addressed by the null-terminated control `name` as `T`.
///
/// The control name is resolved on every call. Use [`read_mib`] with a cached
/// MIB for repeated access.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the name, access, or output size.
///
/// # Panics
///
/// Panics if `name` is empty or does not end in NUL, or if jemalloc succeeds
/// without reporting exactly `size_of::<T>()` output bytes.
///
/// # Safety
///
/// `T` must be layout-compatible with the selected control's output type, and
/// every successful output must be a valid `T`. A same-sized type with stricter
/// validity requirements, such as `u8` read as `bool`, can cause undefined
/// behavior.
///
/// `arena.<i>.name` instead treats the read slot as a pointer to a
/// caller-allocated buffer of at least 32 bytes (`ARENA_NAME_LEN`). Reading
/// that control as `*const c_char` lets jemalloc write through an uninitialized
/// pointer.
pub unsafe fn read<T: Copy>(name: &[u8]) -> Result<T> {
	unsafe {
		validate_name(name);

		let mut len = size_of::<T>();
		let mut value = MaybeUninit { init: () };
		let value_ptr = (&raw mut value.init).cast();

		cvt(jevmalloc_sys::mallctl(
			ptr::from_ref(name).cast::<c_char>(),
			value_ptr,
			&raw mut len,
			ptr::null_mut(),
			0,
		))?;
		assert_eq!(len, size_of::<T>());
		Ok(value.maybe_uninit)
	}
}

/// Writes `value` to the control addressed by `mib`.
///
/// Use [`name_to_mib`] to translate a control name when the same key will be
/// written repeatedly.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the MIB, access, or input size.
///
/// # Safety
///
/// `T` must be layout-compatible with the selected control's input type, and
/// `value` must be valid for that type. Jemalloc reads the supplied bytes as
/// its declared value type. For a pointer-valued control, the pointee must meet
/// every control-specific accessibility, aliasing, and lifetime requirement;
/// any pointer retained by jemalloc must remain valid for that entire period.
pub unsafe fn write_mib<T>(mib: &[usize], mut value: T) -> Result<()> {
	unsafe {
		let len = size_of::<T>();
		let value_ptr = if len > 0 {
			(&raw mut value).cast()
		} else {
			ptr::null_mut()
		};

		cvt(jevmalloc_sys::mallctlbymib(
			mib.as_ptr(),
			mib.len(),
			ptr::null_mut(),
			ptr::null_mut(),
			value_ptr,
			len,
		))
	}
}

/// Writes `value` to the null-terminated control `name`.
///
/// The control name is resolved on every call. Use [`write_mib`] with a cached
/// MIB for repeated access.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the name, access, or input size.
///
/// # Panics
///
/// Panics if `name` is empty or does not end in NUL.
///
/// # Safety
///
/// `T` must be layout-compatible with the selected control's input type, and
/// `value` must be valid for that type. Jemalloc reads the supplied bytes as
/// its declared value type. For a pointer-valued control, the pointee must meet
/// every control-specific accessibility, aliasing, and lifetime requirement;
/// any pointer retained by jemalloc must remain valid for that entire period.
pub unsafe fn write<T>(name: &[u8], mut value: T) -> Result<()> {
	unsafe {
		validate_name(name);

		let len = size_of::<T>();
		let value_ptr = if len > 0 {
			(&raw mut value).cast()
		} else {
			ptr::null_mut()
		};

		cvt(jevmalloc_sys::mallctl(
			ptr::from_ref(name).cast::<c_char>(),
			ptr::null_mut(),
			ptr::null_mut(),
			value_ptr,
			len,
		))
	}
}

/// Replaces the value addressed by `mib` and returns its previous value as `T`.
///
/// Use [`name_to_mib`] to translate a control name when the same key will be
/// updated repeatedly.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the MIB, access, or value size.
///
/// # Panics
///
/// Panics only if the internal `size_of::<T>()` consistency assertion fails.
///
/// # Safety
///
/// `T` must be layout-compatible with both the selected control's input and
/// output types. `in_value` must be a valid input representation, and every
/// successful call must write exactly `size_of::<T>()` initialized bytes that
/// form a valid `T`. Pointer inputs and outputs must meet all control-specific
/// pointee accessibility, aliasing, and lifetime requirements.
pub unsafe fn update_mib<T: Copy>(mib: &[usize], mut in_value: T) -> Result<T> {
	unsafe {
		let in_len = size_of::<T>();
		let in_value_ptr = if in_len > 0 {
			(&raw mut in_value).cast()
		} else {
			ptr::null_mut()
		};

		let mut out_len = in_len;
		let mut out_value = MaybeUninit { init: () };
		let out_value_ptr = if out_len > 0 {
			(&raw mut out_value.init).cast()
		} else {
			ptr::null_mut()
		};

		let out_len_ptr = if out_len > 0 {
			(&raw mut out_len).cast()
		} else {
			ptr::null_mut()
		};

		cvt(jevmalloc_sys::mallctlbymib(
			mib.as_ptr(),
			mib.len(),
			out_value_ptr,
			out_len_ptr,
			in_value_ptr,
			in_len,
		))?;
		assert_eq!(in_len, size_of::<T>());
		Ok(out_value.maybe_uninit)
	}
}

/// Replaces the value addressed by `name` and returns its previous value as
/// `T`.
///
/// The null-terminated control name is resolved on every call. Use
/// [`update_mib`] with a cached MIB for repeated access.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the name, access, or value size.
///
/// # Panics
///
/// Panics if `name` is empty or does not end in NUL, or if jemalloc succeeds
/// without reporting exactly `size_of::<T>()` output bytes.
///
/// # Safety
///
/// `T` must be layout-compatible with both the selected control's input and
/// output types. `in_value` must be a valid input representation, and every
/// successful output must be a valid `T`. Pointer inputs and outputs must meet
/// all control-specific pointee accessibility, aliasing, and lifetime
/// requirements.
pub unsafe fn update<T: Copy>(name: &[u8], mut in_value: T) -> Result<T> {
	unsafe {
		validate_name(name);

		let in_len = size_of::<T>();
		let in_value_ptr = if in_len > 0 {
			(&raw mut in_value).cast()
		} else {
			ptr::null_mut()
		};

		let mut out_len = in_len;
		let mut out_value = MaybeUninit { init: () };
		let out_value_ptr = if out_len > 0 {
			(&raw mut out_value.init).cast()
		} else {
			ptr::null_mut()
		};

		let out_len_ptr = if out_len > 0 {
			(&raw mut out_len).cast()
		} else {
			ptr::null_mut()
		};

		cvt(jevmalloc_sys::mallctl(
			ptr::from_ref(name).cast::<c_char>(),
			out_value_ptr,
			out_len_ptr,
			in_value_ptr,
			in_len,
		))?;
		assert_eq!(out_len, size_of::<T>());
		Ok(out_value.maybe_uninit)
	}
}

/// Reads the NUL-terminated byte string pointer addressed by `mib`.
///
/// The returned slice includes its trailing NUL byte and carries a static
/// lifetime. Use [`name_to_mib`] to cache the MIB for repeated access.
/// `arena.<i>.name` is excluded because its read slot contains a caller-owned
/// buffer of at least 32 bytes rather than an output pointer.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the MIB, access, or pointer size.
///
/// # Panics
///
/// Panics if jemalloc returns a null pointer or succeeds with an unexpected
/// pointer size.
///
/// # Safety
///
/// The selected control must return a non-null pointer to readable,
/// NUL-terminated bytes. Its storage must remain valid and immutable for the
/// lifetime of the returned reference; scanning an invalid or unterminated
/// pointer is undefined behavior.
pub unsafe fn read_str_mib(mib: &[usize]) -> Result<&'static [u8]> {
	unsafe {
		let ptr: *const c_char = read_mib(mib)?;
		Ok(ptr2str(ptr))
	}
}

/// Writes a static NUL-terminated byte string pointer to `mib`.
///
/// Use [`name_to_mib`] when the same key will be written repeatedly.
///
/// # Warning
///
/// The MIB must identify a control whose input is a C string pointer. This safe
/// signature cannot validate the numeric MIB; selecting another pointer-sized
/// control can make jemalloc dereference or retain an invalid pointer.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the MIB, access, or pointer size.
///
/// # Panics
///
/// Panics if `value` is empty or does not end in NUL.
pub fn write_str_mib(mib: &[usize], value: &'static [u8]) -> Result<()> {
	assert!(!value.is_empty(), "value cannot be empty");
	assert_eq!(*value.last().unwrap(), b'\0');
	// The validated static bytes satisfy the C string pointer passed to the raw
	// writer.
	unsafe { write_mib(mib, value.as_ptr().cast::<c_char>()) }
}

/// Replaces the string pointer addressed by `mib` and returns its previous
/// bytes.
///
/// The returned slice includes its trailing NUL byte and carries a static
/// lifetime. Use [`name_to_mib`] to cache the MIB for repeated access.
/// `arena.<i>.name` is excluded because its old-value slot contains a
/// caller-owned buffer of at least 32 bytes rather than an output pointer.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the MIB, access, or pointer size.
///
/// # Panics
///
/// Panics if jemalloc returns a null previous-value pointer or the internal
/// fixed-size consistency assertion fails.
///
/// # Safety
///
/// `value` must end in NUL, and the selected control must accept its pointer as
/// the new value. The control must return a non-null pointer to readable,
/// NUL-terminated bytes whose storage remains valid and immutable for the
/// lifetime of the returned reference.
pub unsafe fn update_str_mib(mib: &[usize], value: &'static [u8]) -> Result<&'static [u8]> {
	unsafe {
		let ptr: *const c_char = update_mib(mib, value.as_ptr().cast::<c_char>())?;
		Ok(ptr2str(ptr))
	}
}

/// Reads the NUL-terminated byte string pointer addressed by `name`.
///
/// The returned slice includes its trailing NUL byte and carries a static
/// lifetime. The null-terminated control name is resolved on every call.
/// `arena.<i>.name` is excluded because its read slot contains a caller-owned
/// buffer of at least 32 bytes rather than an output pointer.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the name, access, or pointer size.
///
/// # Panics
///
/// Panics if `name` is empty or does not end in NUL, if jemalloc returns a null
/// pointer, or if a successful read reports an unexpected pointer size.
///
/// # Safety
///
/// The selected control must return a non-null pointer to readable,
/// NUL-terminated bytes. Its storage must remain valid and immutable for the
/// lifetime of the returned reference; scanning an invalid or unterminated
/// pointer is undefined behavior.
pub unsafe fn read_str(name: &[u8]) -> Result<&'static [u8]> {
	unsafe {
		let ptr: *const c_char = read(name)?;
		Ok(ptr2str(ptr))
	}
}

/// Writes a static NUL-terminated byte string pointer to `name`.
///
/// Use [`write_str_mib`] with a cached MIB for repeated access.
///
/// # Warning
///
/// The name must identify a control whose input is a C string pointer. This
/// safe signature does not validate the control's value type; selecting
/// another pointer-sized control can make jemalloc dereference or retain an
/// invalid pointer.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the name, access, or pointer size.
///
/// # Panics
///
/// Panics if `name` or `value` is empty or does not end in NUL.
pub fn write_str(name: &[u8], value: &'static [u8]) -> Result<()> {
	assert!(!value.is_empty(), "value cannot be empty");
	assert_eq!(*value.last().unwrap(), b'\0');
	// The validated static bytes satisfy the C string pointer passed to the raw
	// writer.
	unsafe { write(name, value.as_ptr().cast::<c_char>()) }
}

/// Replaces the string pointer addressed by `name` and returns its previous
/// bytes.
///
/// The returned slice includes its trailing NUL byte and carries a static
/// lifetime. The null-terminated control name is resolved on every call.
/// `arena.<i>.name` is excluded because its old-value slot contains a
/// caller-owned buffer of at least 32 bytes rather than an output pointer.
///
/// # Errors
///
/// Returns an error when jemalloc rejects the name, access, or pointer size.
///
/// # Panics
///
/// Panics if `name` is empty or does not end in NUL, if jemalloc returns a null
/// previous-value pointer, or if a successful read reports an unexpected size.
///
/// # Safety
///
/// `value` must end in NUL, and the selected control must accept its pointer as
/// the new value. The control must return a non-null pointer to readable,
/// NUL-terminated bytes whose storage remains valid and immutable for the
/// lifetime of the returned reference.
pub unsafe fn update_str(name: &[u8], value: &'static [u8]) -> Result<&'static [u8]> {
	unsafe {
		let ptr: *const c_char = update(name, value.as_ptr().cast::<c_char>())?;
		Ok(ptr2str(ptr))
	}
}

/// Builds a byte slice, including its NUL terminator, from a C string pointer.
///
/// The scan uses `strlen`, and no UTF-8 validation is performed. The returned
/// slice carries a static lifetime.
///
/// # Panics
///
/// Panics if `ptr` is null.
///
/// # Safety
///
/// `ptr` must be readable through its first NUL byte. The referenced storage
/// must remain valid and immutable for the lifetime of the returned slice.
unsafe fn ptr2str(ptr: *const c_char) -> &'static [u8] {
	unsafe {
		assert!(!ptr.is_null(), "attempt to convert a null-ptr to a UTF-8 string");
		let len = libc::strlen(ptr);
		slice::from_raw_parts(ptr.cast::<u8>(), len + 1)
	}
}

/// Checks that a control name is nonempty and NUL-terminated.
///
/// Embedded NUL bytes are accepted because only the final byte is validated.
///
/// # Panics
///
/// Panics if `name` is empty or its final byte is not NUL.
fn validate_name(name: &[u8]) {
	assert!(!name.is_empty(), "empty byte string");
	assert_eq!(*name.last().unwrap(), b'\0', "non-null terminated byte string");
}

/// Provides stack storage that C may initialize as `T`.
///
/// The unit field permits construction without first creating a `T`. Reading
/// the value field is valid only after the FFI operation fully initializes it.
union MaybeUninit<T: Copy> {
	/// Constructs the storage without initializing a `T`.
	///
	/// The value field remains unreadable until C writes a valid `T` into the
	/// same storage.
	init: (),

	/// Holds the value written by C.
	///
	/// Reading this field requires proof that every byte and validity invariant
	/// of `T` was initialized.
	maybe_uninit: T,
}

#[cfg(test)]
/// Tests private conversions used by the raw control accessors.
///
/// Coverage focuses on preserving the trailing NUL byte in returned slices.
mod tests {
	use super::*;

	/// Verifies that `ptr2str` includes the trailing NUL byte.
	///
	/// The cases cover an empty C string and a string containing repeated
	/// spaces.
	#[test]
	#[cfg(not(target_arch = "mips64"))] // Disabled because this test triggers SIGFPE.
	fn test_ptr2str() {
		unsafe {
			let empty = ptr2str(c"".as_ptr());

			assert_eq!(empty.len(), 1);
			assert_eq!(empty, b"\0");

			let padded = ptr2str(c"foo  baaar".as_ptr());

			assert_eq!(padded.len(), b"foo  baaar\0".len());
			assert_eq!(padded, b"foo  baaar\0");
		}
	}
}

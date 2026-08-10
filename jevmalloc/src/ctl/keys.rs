//! Typed keys for jemalloc's `mallctl` namespace.
//!
//! [`Name`] borrows a null-terminated control name, while [`Mib`] and
//! [`MibStr`] hold its resolved numeric components. [`Access`] reads and
//! mutates the value selected by either representation.
//!
//! # Examples
//!
//! The numeric components of a MIB may be adjusted to query related controls
//! without resolving each name again:
//!
//! ```
//! #[global_allocator]
//! static ALLOC: jevmalloc::Jemalloc = jevmalloc::Jemalloc;
//!
//! fn main() {
//! 	use jevmalloc::ctl::{Access, AsName, Mib};
//! 	use libc::c_uint;
//! 	let name = b"arenas.nbins\0".name();
//! 	let nbins: c_uint = name.read().unwrap();
//! 	let mut mib: Mib<[usize; 4]> = b"arenas.bin.0.size\0".name().mib().unwrap();
//! 	for i in 0..4 {
//! 		mib[2] = i;
//! 		let bin_size: usize = mib.read().unwrap();
//! 		println!("arena bin {} has size {}", i, bin_size);
//! 	}
//! }
//! ```

#![allow(clippy::uninlined_format_args)]

use super::{Result, raw};
use crate::std::{fmt, ops};

/// A borrowed name in jemalloc's `mallctl` namespace.
///
/// The underlying byte string includes its terminating null byte. Use
/// [`AsName::name`] to borrow a string or byte slice in this form.
#[repr(transparent)]
#[derive(PartialEq, Eq)]
pub struct Name(
	/// The control name bytes, including the terminating null byte.
	[u8],
);

/// Borrows a null-terminated string or byte slice as a control [`Name`].
///
/// This conversion does not allocate or resolve the name with jemalloc.
pub trait AsName {
	/// Returns this value as a control name.
	///
	/// The value must contain at least its terminating null byte.
	///
	/// # Panics
	///
	/// Panics if the value is empty or does not end in a null byte.
	fn name(&self) -> &Name;
}

impl AsName for [u8] {
	fn name(&self) -> &Name {
		assert!(!self.is_empty(), "cannot create Name from empty byte-string");
		assert_eq!(
			*self.last().unwrap(),
			b'\0',
			"cannot create Name from non-null-terminated byte-string \"{}\"",
			str::from_utf8(self).unwrap()
		);
		unsafe { &*(core::ptr::from_ref::<Self>(self) as *const Name) }
	}
}

impl AsName for str {
	fn name(&self) -> &Name { self.as_bytes().name() }
}

impl Name {
	/// Resolves this name into a [`Mib`] intended for non-string access.
	///
	/// The slice exposed by `T` determines how many numeric components are
	/// resolved. A shorter slice produces a partial MIB. The name's value type
	/// is not validated; callers must pair the result with the matching
	/// [`Access`] implementation.
	///
	/// # Errors
	///
	/// Returns [`Error`](super::Error) if jemalloc rejects the name or output
	/// buffer.
	///
	/// # Panics
	///
	/// Panics if jemalloc reports a component count different from the length
	/// of the slice exposed by `T`.
	pub fn mib<T: MibArg>(&self) -> Result<Mib<T>> {
		let mut mib: Mib<T> = Mib::default();
		raw::name_to_mib(&self.0, mib.0.as_mut())?;
		Ok(mib)
	}

	/// Resolves this name into a [`MibStr`] for a string value.
	///
	/// The slice exposed by `T` determines how many numeric components are
	/// resolved.
	///
	/// # Errors
	///
	/// Returns [`Error`](super::Error) if jemalloc rejects the name or output
	/// buffer.
	///
	/// # Panics
	///
	/// Panics if this wrapper does not recognize the name as string-valued, or
	/// if jemalloc reports a component count different from the length of the
	/// slice exposed by `T`.
	pub fn mib_str<T: MibArg>(&self) -> Result<MibStr<T>> {
		assert!(self.value_type_str(), "key \"{}\" does not refer to a string", self);
		let mut mib: MibStr<T> = MibStr::default();
		raw::name_to_mib(&self.0, mib.0.as_mut())?;
		Ok(mib)
	}

	/// Reports whether this wrapper recognizes the control as string-valued.
	///
	/// The recognized set consists of jemalloc controls whose values are
	/// pointers to null-terminated strings.
	///
	/// # Panics
	///
	/// Panics if the name has no bytes. In debug builds, it also panics if the
	/// byte immediately before the terminating null byte is itself null.
	#[must_use]
	pub fn value_type_str(&self) -> bool {
		// remove the null-terminator:
		let name = self.0.split_at(self.0.len() - 1).0;
		if name.is_empty() {
			return false;
		}
		debug_assert_ne!(*name.last().unwrap(), b'\0');

		match name {
			| b"version"
			| b"config.malloc_conf"
			| b"opt.metadata_thp"
			| b"opt.dss"
			| b"opt.percpu_arena"
			| b"opt.stats_print_opts"
			| b"opt.junk"
			| b"opt.thp"
			| b"opt.prof_prefix"
			| b"thread.prof.name"
			| b"prof.dump" => true,
			| v if v.starts_with(b"arena.") && v.ends_with(b".dss") => true,
			| v if v.starts_with(b"stats.arenas.") && v.ends_with(b".dss") => true,
			| _ => false,
		}
	}

	/// Returns the control name bytes, including the terminating null byte.
	///
	/// # Warning
	///
	/// The returned reference is typed as static even when this [`Name`]
	/// borrows shorter-lived storage. Do not retain the slice beyond the
	/// lifetime of the original name bytes.
	#[must_use]
	pub fn as_bytes(&self) -> &'static [u8] {
		unsafe { &*(core::ptr::from_ref::<Self>(self) as *const [u8]) }
	}
}

impl fmt::Debug for Name {
	/// Formats the control name as UTF-8 text.
	///
	/// # Panics
	///
	/// Panics if the name contains bytes that are not valid UTF-8.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", str::from_utf8(&self.0).unwrap())
	}
}

impl fmt::Display for Name {
	/// Writes the control name as UTF-8 text.
	///
	/// # Panics
	///
	/// Panics if the name contains bytes that are not valid UTF-8.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", str::from_utf8(&self.0).unwrap())
	}
}

/// A resolved MIB key intended for non-string control access.
///
/// `T` stores the numeric components returned by jemalloc. Mutable indexing
/// allows a component such as an arena or size-class index to be reused for a
/// related control. [`Default`] only initializes the component storage; use
/// [`Name::mib`] to resolve a key before accessing it. Neither construction nor
/// access validates the selected control's value type.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Mib<T: MibArg>(
	/// The numeric components of the resolved control name.
	T,
);

/// A resolved MIB key whose accessors interpret the value as a string.
///
/// [`Default`] only initializes the component storage. Use [`Name::mib_str`] to
/// resolve a key before access, and ensure its components continue to select a
/// jemalloc control using the ordinary string-pointer convention.
/// `arena.<i>.name`, which reads into a caller-owned buffer, is not compatible
/// with these accessors.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct MibStr<T: MibArg>(
	/// The numeric components of the resolved string control name.
	T,
);

impl<T: MibArg> AsRef<[usize]> for Mib<T> {
	fn as_ref(&self) -> &[usize] { self.0.as_ref() }
}

impl<T: MibArg> AsMut<[usize]> for Mib<T> {
	fn as_mut(&mut self) -> &mut [usize] { self.0.as_mut() }
}

impl<T: MibArg> ops::Index<usize> for Mib<T> {
	type Output = usize;

	fn index(&self, idx: usize) -> &Self::Output { &self.0.as_ref()[idx] }
}

impl<T: MibArg> ops::IndexMut<usize> for Mib<T> {
	fn index_mut(&mut self, idx: usize) -> &mut Self::Output { &mut self.0.as_mut()[idx] }
}

impl<T: MibArg> ops::Index<usize> for MibStr<T> {
	type Output = usize;

	fn index(&self, idx: usize) -> &Self::Output { &self.0.as_ref()[idx] }
}

impl<T: MibArg> ops::IndexMut<usize> for MibStr<T> {
	fn index_mut(&mut self, idx: usize) -> &mut Self::Output { &mut self.0.as_mut()[idx] }
}

/// Reads and mutates a jemalloc control as the Rust value type `T`.
///
/// Implementations cover the scalar and string representations supported by
/// this wrapper. The chosen `T` must match the value represented by the name or
/// MIB; mutable and default MIB values receive no type validation. String
/// inputs must be nonempty and include their terminating null byte, although
/// update paths do not validate that requirement.
///
/// # Warning
///
/// These safe methods rely on value-type and pointer-lifetime invariants that
/// their key types do not enforce. A mismatched type, a retargeted [`MibStr`],
/// or a stale string pointer can violate the underlying foreign-function
/// contract.
///
/// String reads expose jemalloc-owned storage with a static reference type.
/// Copy `thread.prof.name` output promptly because jemalloc can deallocate that
/// string asynchronously, and never retain any returned string past its
/// control-specific lifetime.
pub trait Access<T> {
	/// Reads the value selected by this key.
	///
	/// # Errors
	///
	/// Returns [`Error`](super::Error) if jemalloc rejects the key, the
	/// requested access, or the value representation.
	///
	/// # Panics
	///
	/// Panics if jemalloc returns a value whose size or boolean representation
	/// does not match `T`. String reads panic if the returned pointer is null,
	/// and `str` reads also panic on invalid UTF-8. String access through
	/// [`Name`] additionally panics when the name is not recognized as
	/// string-valued.
	fn read(&self) -> Result<T>;

	/// Writes `value` to the control selected by this key.
	///
	/// # Errors
	///
	/// Returns [`Error`](super::Error) if jemalloc rejects the key, the
	/// requested access, or the value representation.
	///
	/// # Panics
	///
	/// String access through [`Name`] panics when the name is not recognized as
	/// string-valued. Writing a string value also panics when it is empty or
	/// lacks a terminating null byte.
	fn write(&self, value: T) -> Result<()>;

	/// Writes `value` and returns the control's previous value.
	///
	/// # Errors
	///
	/// Returns [`Error`](super::Error) if jemalloc rejects the key, the
	/// requested access, or either value representation.
	///
	/// # Panics
	///
	/// String access through [`Name`] panics when the name is not recognized as
	/// string-valued. Named scalar updates panic on an unexpected output size.
	/// String updates panic if the previous-value pointer is null, and `str`
	/// updates also panic when the previous bytes are not valid UTF-8. MIB
	/// scalar updates do not validate jemalloc's reported output size.
	fn update(&self, value: T) -> Result<T>;
}

/// Implements scalar control access for both MIB and name keys.
macro_rules! impl_access {
	($id:ty) => {
		impl<T: MibArg> Access<$id> for Mib<T> {
			fn read(&self) -> Result<$id> { unsafe { raw::read_mib(self.0.as_ref()) } }

			fn write(&self, value: $id) -> Result<()> {
				unsafe { raw::write_mib(self.0.as_ref(), value) }
			}

			fn update(&self, value: $id) -> Result<$id> {
				unsafe { raw::update_mib(self.0.as_ref(), value) }
			}
		}
		impl Access<$id> for Name {
			fn read(&self) -> Result<$id> { unsafe { raw::read(&self.0) } }

			fn write(&self, value: $id) -> Result<()> { unsafe { raw::write(&self.0, value) } }

			fn update(&self, value: $id) -> Result<$id> { unsafe { raw::update(&self.0, value) } }
		}
	};
}

impl_access!(u32);
impl_access!(u64);
impl_access!(isize);
impl_access!(usize);

impl<T: MibArg> Access<bool> for Mib<T> {
	fn read(&self) -> Result<bool> {
		unsafe {
			let v: u8 = raw::read_mib(self.0.as_ref())?;
			assert!(v == 0 || v == 1);
			Ok(v == 1)
		}
	}

	fn write(&self, value: bool) -> Result<()> {
		unsafe { raw::write_mib(self.0.as_ref(), value) }
	}

	fn update(&self, value: bool) -> Result<bool> {
		unsafe {
			let v: u8 = raw::update_mib(self.0.as_ref(), u8::from(value))?;
			Ok(v == 1)
		}
	}
}

impl Access<bool> for Name {
	fn read(&self) -> Result<bool> {
		unsafe {
			let v: u8 = raw::read(&self.0)?;
			assert!(v == 0 || v == 1);
			Ok(v == 1)
		}
	}

	fn write(&self, value: bool) -> Result<()> { unsafe { raw::write(&self.0, value) } }

	fn update(&self, value: bool) -> Result<bool> {
		unsafe {
			let v: u8 = raw::update(&self.0, u8::from(value))?;
			Ok(v == 1)
		}
	}
}

impl<T: MibArg> Access<&'static [u8]> for MibStr<T> {
	fn read(&self) -> Result<&'static [u8]> {
		// The mutable MIB must still select a string with valid pointer semantics.
		unsafe { raw::read_str_mib(self.0.as_ref()) }
	}

	fn write(&self, value: &'static [u8]) -> Result<()> {
		raw::write_str_mib(self.0.as_ref(), value)
	}

	fn update(&self, value: &'static [u8]) -> Result<&'static [u8]> {
		// The mutable MIB must still select a string with valid pointer semantics.
		unsafe { raw::update_str_mib(self.0.as_ref(), value) }
	}
}

impl Access<&'static [u8]> for Name {
	fn read(&self) -> Result<&'static [u8]> {
		assert!(self.value_type_str(), "the name \"{:?}\" does not refer to a byte string", self);
		// The recognized control must uphold its documented pointer lifetime.
		unsafe { raw::read_str(&self.0) }
	}

	fn write(&self, value: &'static [u8]) -> Result<()> {
		assert!(self.value_type_str(), "the name \"{:?}\" does not refer to a byte string", self);
		raw::write_str(&self.0, value)
	}

	fn update(&self, value: &'static [u8]) -> Result<&'static [u8]> {
		assert!(self.value_type_str(), "the name \"{:?}\" does not refer to a byte string", self);
		// The recognized control must uphold its documented pointer lifetime.
		unsafe { raw::update_str(&self.0, value) }
	}
}

impl<T: MibArg> Access<&'static str> for MibStr<T> {
	fn read(&self) -> Result<&'static str> {
		// The mutable MIB must still select a string with valid pointer semantics.
		let s = unsafe { raw::read_str_mib(self.0.as_ref())? };
		Ok(str::from_utf8(s).unwrap())
	}

	fn write(&self, value: &'static str) -> Result<()> {
		raw::write_str_mib(self.0.as_ref(), value.as_bytes())
	}

	fn update(&self, value: &'static str) -> Result<&'static str> {
		// The mutable MIB must still select a string with valid pointer semantics.
		let s = unsafe { raw::update_str_mib(self.0.as_ref(), value.as_bytes())? };
		Ok(str::from_utf8(s).unwrap())
	}
}

impl Access<&'static str> for Name {
	fn read(&self) -> Result<&'static str> {
		assert!(self.value_type_str(), "the name \"{:?}\" does not refer to a byte string", self);
		// The recognized control must uphold its documented pointer lifetime.
		let s = unsafe { raw::read_str(&self.0)? };
		Ok(str::from_utf8(s).unwrap())
	}

	fn write(&self, value: &'static str) -> Result<()> {
		assert!(self.value_type_str(), "the name \"{:?}\" does not refer to a byte string", self);
		raw::write_str(&self.0, value.as_bytes())
	}

	fn update(&self, value: &'static str) -> Result<&'static str> {
		assert!(self.value_type_str(), "the name \"{:?}\" does not refer to a byte string", self);
		// The recognized control must uphold its documented pointer lifetime.
		let s = unsafe { raw::update_str(&self.0, value.as_bytes())? };
		Ok(str::from_utf8(s).unwrap())
	}
}

/// Storage accepted for the numeric components of a [`Mib`] or [`MibStr`].
///
/// Arrays of `usize` are the usual implementation. Their length selects the
/// number of name components that [`Name::mib`] or [`Name::mib_str`] asks
/// jemalloc to resolve.
pub trait MibArg:
	Copy + Clone + PartialEq + Default + fmt::Debug + AsRef<[usize]> + AsMut<[usize]>
{
}
impl<T> MibArg for T where
	T: Copy + Clone + PartialEq + Default + fmt::Debug + AsRef<[usize]> + AsMut<[usize]>
{
}

#[cfg(test)]
mod tests {
	//! Exercises typed access through names and resolved MIB keys.

	use super::{Access, AsName, Mib, MibStr};

	/// Reads and writes a boolean control through both key representations.
	#[test]
	fn bool_rw() {
		let name = b"thread.tcache.enabled\0".name();
		let tcache: bool = name.read().unwrap();

		let new_tcache = !tcache;

		name.write(new_tcache).unwrap();

		let mib: Mib<[usize; 3]> = name.mib().unwrap();
		let r: bool = mib.read().unwrap();
		assert_eq!(r, new_tcache);
	}

	/// Reads a 32-bit control through both key representations.
	#[test]
	fn u32_r() {
		let name = b"arenas.bin.0.nregs\0".name();
		let v: u32 = name.read().unwrap();

		let mib: Mib<[usize; 4]> = name.mib().unwrap();
		let r: u32 = mib.read().unwrap();
		assert_eq!(r, v);
	}

	/// Reads a `size_t` control through both key representations.
	#[test]
	fn size_t_r() {
		let name = b"arenas.lextent.0.size\0".name();
		let v: libc::size_t = name.read().unwrap();

		let mib: Mib<[usize; 4]> = name.mib().unwrap();
		let r: libc::size_t = mib.read().unwrap();
		assert_eq!(r, v);
	}

	/// Reads and rewrites an `ssize_t` control through both key
	/// representations.
	#[test]
	fn ssize_t_rw() {
		let name = b"arenas.dirty_decay_ms\0".name();
		let v: libc::ssize_t = name.read().unwrap();
		name.write(v).unwrap();

		let mib: Mib<[usize; 2]> = name.mib().unwrap();
		let r: libc::ssize_t = mib.read().unwrap();
		assert_eq!(r, v);
	}

	/// Reads and rewrites a 64-bit control through both key representations.
	#[test]
	fn u64_rw() {
		let name = b"epoch\0".name();
		let epoch: u64 = name.read().unwrap();
		name.write(epoch).unwrap();

		let mib: Mib<[usize; 1]> = name.mib().unwrap();
		let epoch: u64 = mib.read().unwrap();
		mib.write(epoch).unwrap();
	}

	/// Reads and rewrites a string control through both key representations.
	#[test]
	fn str_rw() {
		let name = b"arena.0.dss\0".name();
		let dss: &'static [u8] = name.read().unwrap();
		name.write(dss).unwrap();

		let mib: MibStr<[usize; 3]> = name.mib_str().unwrap();
		let dss2: &'static [u8] = mib.read().unwrap();
		mib.write(dss2).unwrap();

		assert_eq!(dss, dss2);
	}
}

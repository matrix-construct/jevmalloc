#![cfg(test)]

#[cfg(prefixed)]
#[test]
fn malloc_is_prefixed() {
	assert_ne!(jevmalloc_sys::malloc as *const () as usize, libc::malloc as *const () as usize)
}

#[cfg(not(prefixed))]
#[test]
fn malloc_is_overridden() {
	assert_eq!(jevmalloc_sys::malloc as *const () as usize, libc::malloc as *const () as usize);
}

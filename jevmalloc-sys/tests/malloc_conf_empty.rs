#![cfg(test)]

#[test]
fn malloc_conf_empty() {
	unsafe {
		assert!(jevmalloc_sys::malloc_conf.is_none());
	}
}

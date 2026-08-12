//! Calling-thread controls and explicit thread-cache handles.
//!
//! [`this`] contains state and commands for the calling thread, including its
//! automatically managed allocation cache. [`ThreadCache`] instead owns one
//! explicitly created cache selected through extended-allocation flags.

pub mod cache;
#[cfg(feature = "stats")]
pub mod counters;
pub mod this;

pub use self::cache::{ThreadCache, ThreadCacheDestroyError};
#[cfg(feature = "stats")]
pub use self::counters::ThreadCounters;

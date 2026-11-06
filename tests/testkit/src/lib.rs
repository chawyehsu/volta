//! Self-contained test kit for Volta's acceptance and smoke tests.
//!
//! Provides the sandboxed [`sandbox::Sandbox`] test environment, the
//! process/assertion harness ported from cargo, and filesystem helpers.

#[macro_export]
macro_rules! ok_or_panic {
    { $e:expr } => {
        match $e {
            Ok(x) => x,
            Err(err) => panic!("{} failed with {}", stringify!($e), err),
        }
    };
}

pub mod fs;
pub mod matchers;
pub mod process;
pub mod sandbox;

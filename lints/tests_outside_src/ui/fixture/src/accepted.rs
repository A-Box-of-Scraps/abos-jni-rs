// compile-flags: --test
#![deny(tests_outside_src)]
#![allow(dead_code)]

struct Subject;

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod relocated;

// Inline path attributes denote directories, not files.
#[path = "../tests"]
mod nested {
    struct Subject;
    #[cfg(test)]
    #[path = "unit/mod.rs"]
    mod relocated;
}

// Ordinary inline modules add their names to the module search directory.
mod ordinary {
    struct Subject;
    #[cfg(test)]
    #[path = "../../tests/unit/mod.rs"]
    mod relocated;
}

#[cfg(not(test))]
fn production_only() {}

#[cfg(any(test, unix))]
fn shared() {}

macro_rules! generated_test {
    () => {
        #[test]
        fn generated() {}
    };
}
generated_test!();

#[allow(tests_outside_src)]
#[test]
fn explicit_exception() {}

fn main() {}

mod auxiliary;

#[cfg_attr(test, allow(dead_code))]
fn conditionally_allowed() {}

#[cfg(any(test, feature = "shared"))]
fn shared_feature() {}

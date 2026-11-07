// compile-flags: --test
#![allow(dead_code)]

#[test]
fn standalone_test() {}

#[cfg(test)]
mod inline_tests {
    fn compile_only() {}
}

// Inactive predicates are visible before expansion in a parsed file.
#[cfg(all(test, windows))]
mod windows_tests {
    fn compile_only() {}
}

#[cfg(test)]
const _: () = assert!(true);

macro_rules! compile_assertion {
    () => { const _: () = assert!(true); };
}
#[cfg(test)]
compile_assertion!();

struct Subject;
impl Subject {
    #[cfg(test)]
    fn helper() {}
}

#[cfg_attr(test, test)]
fn conditional_test() {}

#[cfg(not(test))]
fn production_only() {}

#[cfg(any(test, unix))]
fn shared_with_production() {}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod relocated;

#[cfg(test)]
mod misplaced;

/// Documentation examples remain permitted.
/// ```
/// assert_eq!(1 + 1, 2);
/// ```
fn documented() {}

fn main() {}

// A path attribute is not an exemption if it resolves back into src/.
#[cfg(test)]
#[path = "../tests/../src/misplaced.rs"]
mod disguised;

#[cfg(not(not(test)))]
fn double_negation() {}

#[cfg_attr(test, cfg(test))]
fn conditional_helper() {}

#[cfg(all(test, any(unix, windows)))]
fn platform_helper() {}

// Custom test attributes are recognized without requiring the test framework.
#[cfg(any())]
#[example_framework::test]
async fn qualified_test() {}

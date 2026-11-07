// compile-flags: --test
#![deny(tests_outside_src)]

// A src directory below tests/ is not the package's production src/.
#[cfg(test)]
mod tests {
    #[test]
    fn integration_test() {
        assert_eq!(2 + 2, 4);
    }
}

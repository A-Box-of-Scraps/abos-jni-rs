// Test code must be rejected even without --test.
#[test]
fn standalone() {}

#[cfg(test)]
mod tests {
    fn compile_only() {}
}

// No false positive for a test-only declaration whose file is not loaded.
#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod relocated;

fn main() {}

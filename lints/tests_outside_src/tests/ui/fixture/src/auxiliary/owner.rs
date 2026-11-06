mod nested {
    struct Subject;
    #[cfg(test)]
    #[path = "../../../../tests/unit/mod.rs"]
    mod relocated;
}

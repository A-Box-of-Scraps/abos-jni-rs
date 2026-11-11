use std::{env, ffi::OsStr};

pub fn arguments(include: &OsStr) -> Vec<String> {
	env::split_paths(include)
		// A bare /I consumes the next argument, including compiler control flags.
		.filter(|path| !path.as_os_str().is_empty())
		.map(|path| format!("/I{}", path.display()))
		.collect()
}

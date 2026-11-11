#[path = "../support/include_paths.rs"]
mod include_paths;

use std::{env, ffi::OsStr};

#[test]
fn empty_include_entries_do_not_consume_compiler_flags() {
	let include = env::join_paths(["", "MSVC include", "", "Windows SDK/include", ""]).expect("valid INCLUDE paths");
	assert_eq!(
		include_paths::arguments(&include),
		["/IMSVC include", "/IWindows SDK/include"]
	);
}

#[test]
fn empty_include_list_adds_no_flags() {
	assert_eq!(include_paths::arguments(OsStr::new("")), Vec::<String>::new());
}

#[test]
fn normal_include_paths_keep_their_order() {
	let include = env::join_paths(["first/include", "second/include"]).expect("valid INCLUDE paths");
	assert_eq!(
		include_paths::arguments(&include),
		["/Ifirst/include", "/Isecond/include"]
	);
}

use std::{env, ffi::OsStr, fs, path::Path};

pub fn write(build: &cc::Build, out: &Path) {
	let directory = env::current_dir().expect("build directory");
	let compiler = build.get_compiler();
	let mut arguments = vec![compiler.path().to_string_lossy().into_owned()];
	arguments.extend(compiler.args().iter().map(|arg| arg.to_string_lossy().into_owned()));
	if compiler.is_like_msvc() {
		// MSVC's system headers are normally configured through the environment, not command-line flags.
		let include = compiler
			.get_envs()
			.find(|(key, _)| key.eq_ignore_ascii_case(OsStr::new("INCLUDE")))
			.map(|(_, value)| value.to_owned())
			.or_else(|| env::var_os("INCLUDE"));
		if let Some(include) = include {
			arguments.extend(env::split_paths(&include).map(|path| format!("/I{}", path.display())));
		}
	}
	arguments.push("-c".to_owned());
	let commands: Vec<_> = build
		.get_files()
		.map(|file| {
			let mut arguments = arguments.clone();
			arguments.push(file.to_string_lossy().into_owned());
			serde_json::json!({
				"directory": directory,
				"file": directory.join(file),
				"arguments": arguments,
			})
		})
		.collect();
	let json = serde_json::to_vec_pretty(&commands).expect("serialize compilation database");
	fs::write(out.join("compile_commands.json"), json).expect("write compilation database");
}

mod options;
use abos_jbindgen::{Bindings, Builder};
use clap::Parser;
use options::{Cli, Command, Options};
use std::{
	fs,
	io::{self, Write},
	path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn run() -> Result<()> {
	let cli: Cli = Cli::parse();
	let (builder, options) = match cli.command {
		Command::Annotations { output } => {
			abos_jbindgen_annotations::write_sources(output)?;
			return Ok(());
		}
		Command::Classfile { input, options } => {
			let builder: Builder = if input.extension().is_some_and(|ext| ext == "jar") {
				Builder::new().input_jar(input)
			} else {
				Builder::new().input_class(input)
			};
			(builder, options)
		}
		Command::Java {
			inputs,
			classpaths,
			options,
		} => {
			let mut sources: Vec<PathBuf> = Vec::new();
			for input in inputs {
				collect_sources(&input, &mut sources)?;
			}
			sources.sort();
			sources.dedup();
			(
				Builder::new().input_sources(sources, classpaths, options.patterns.clone()),
				options,
			)
		}
		Command::Android {
			api_level,
			hiddenapi_flags,
			allow_unsupported,
			max_target,
			options,
		} => {
			let mut builder: Builder = Builder::new()
				.input_android_sdk(api_level, options.patterns.clone())
				.allow_unsupported(allow_unsupported);
			if let Some(flags) = hiddenapi_flags {
				builder = builder.hiddenapi_flags(flags);
			}
			if let Some(target) = max_target {
				builder = builder.max_target(target);
			}
			(builder, options)
		}
	};
	let bindings: Bindings = configure(builder, &options)?.generate()?;
	write_output(&bindings, &options)
}

fn configure(mut builder: Builder, options: &Options) -> Result<Builder> {
	builder = builder
		.root_path(&options.root)
		.private_type(options.private)
		.generate_native_interfaces(!options.no_native_interfaces)
		.generate_jni_init(!options.no_jni_init)
		.patterns(options.patterns.clone());
	if let Some(name) = &options.rust_name {
		builder = builder.rust_type_name(name);
	}
	if let Some(prefix) = &options.name_prefix {
		builder = builder.name_prefix(prefix);
	}
	for signature in &options.skip_signatures {
		builder = builder.skip_signature(signature);
	}
	for entry in &options.name_overrides {
		let (signature, name) = entry.split_once('=').ok_or("name override must be SIGNATURE=NAME")?;
		builder = builder.name_override(signature, name);
	}
	for entry in &options.type_maps {
		if Path::new(entry).is_file() {
			for line in fs::read_to_string(entry)?
				.lines()
				.map(str::trim)
				.filter(|line| !line.is_empty() && !line.starts_with('#'))
			{
				builder = add_mapping(builder, line)?;
			}
		} else {
			builder = add_mapping(builder, entry)?;
		}
	}
	Ok(builder)
}

fn add_mapping(builder: Builder, entry: &str) -> Result<Builder> {
	let (rust, java) = entry
		.split_once("=>")
		.ok_or("type mapping must be RustType=>java.Type")?;
	Ok(builder.type_mapping(rust.trim(), java.trim().trim_matches('"')))
}

fn write_output(bindings: &Bindings, options: &Options) -> Result<()> {
	if let Some(directory) = &options.output_dir {
		bindings.write_to_files(directory)?;
	} else if let Some(file) = &options.output {
		bindings.write_to_file(file)?;
	} else {
		writeln!(io::stdout().lock(), "{bindings}")?;
	}
	if let Some(file) = &options.output_type_map {
		if let Some(root) = &options.public_root {
			bindings.write_pub_type_map(file, root)?;
		} else {
			bindings.write_type_map(file)?;
		}
	}
	Ok(())
}

fn collect_sources(path: &Path, sources: &mut Vec<PathBuf>) -> io::Result<()> {
	if !path.is_dir() {
		sources.push(path.to_owned());
		return Ok(());
	}
	for entry in fs::read_dir(path)? {
		let entry: fs::DirEntry = entry?;
		let kind: fs::FileType = entry.file_type()?;
		let path: PathBuf = entry.path();
		if kind.is_dir() {
			collect_sources(&path, sources)?;
		} else if kind.is_file() && path.extension().is_some_and(|ext| ext == "java") {
			sources.push(path);
		}
	}
	Ok(())
}

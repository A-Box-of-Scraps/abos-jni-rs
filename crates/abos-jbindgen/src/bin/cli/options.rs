use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "abos-jbindgen", version, about = "Generate Rust JNI bindings")]
pub struct Cli {
	#[command(subcommand)]
	pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
	/// Generate bindings from a class file or JAR.
	Classfile {
		input: PathBuf,
		#[command(flatten)]
		options: Options,
	},
	/// Generate bindings from Java sources using the JDK compiler API.
	Java {
		#[arg(required = true)]
		inputs: Vec<PathBuf>,
		#[arg(long = "classpath")]
		classpaths: Vec<PathBuf>,
		#[command(flatten)]
		options: Options,
	},
	/// Generate bindings from an installed Android SDK.
	Android {
		#[arg(long, default_value_t = 35)]
		api_level: u32,
		#[arg(long)]
		hiddenapi_flags: Option<PathBuf>,
		#[arg(long)]
		allow_unsupported: bool,
		#[arg(long)]
		max_target: Option<String>,
		#[command(flatten)]
		options: Options,
	},
	/// Export annotation sources into their Java package directory.
	Annotations {
		#[arg(short, long, default_value = ".")]
		output: PathBuf,
	},
}

#[derive(Args)]
pub struct Options {
	#[arg(long, default_value = "crate")]
	pub root: String,
	#[arg(short, long, conflicts_with = "output_dir")]
	pub output: Option<PathBuf>,
	#[arg(long)]
	pub output_dir: Option<PathBuf>,
	#[arg(long)]
	pub rust_name: Option<String>,
	#[arg(long)]
	pub name_prefix: Option<String>,
	#[arg(long)]
	pub private: bool,
	#[arg(long = "pattern")]
	pub patterns: Vec<String>,
	/// RustType=>java.Type entries or files containing those entries.
	#[arg(long = "type-map")]
	pub type_maps: Vec<String>,
	#[arg(long)]
	pub no_native_interfaces: bool,
	#[arg(long)]
	pub no_jni_init: bool,
	#[arg(long = "skip")]
	pub skip_signatures: Vec<String>,
	/// `DEX_SIGNATURE=rust_name` override.
	#[arg(long = "name")]
	pub name_overrides: Vec<String>,
	#[arg(long)]
	pub output_type_map: Option<PathBuf>,
	#[arg(long)]
	pub public_root: Option<String>,
}

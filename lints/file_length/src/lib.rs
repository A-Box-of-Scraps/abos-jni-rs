#![feature(rustc_private, default_field_values)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use rustc_ast::{Crate, Inline, Item, ItemKind, ModKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::{FileName, SourceFile, Span};

const MAX_LINES: usize = 600;

dylint_linting::impl_early_lint! {
	/// Keeps physical Rust source files within 600 lines, including comments and blank lines.
	pub FILE_LENGTH,
	Warn,
	"Rust source files should not exceed 600 lines",
	FileLength::default()
}

#[derive(Default)]
struct FileLength {
	checked: HashSet<PathBuf>,
}

impl EarlyLintPass for FileLength {
	fn check_crate(&mut self, cx: &EarlyContext<'_>, krate: &Crate) {
		self.check_span(cx, krate.spans.inner_span);
	}

	fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
		if let ItemKind::Mod(_, _, ModKind::Loaded(_, Inline::No { .. }, spans)) = &item.kind {
			// Check at the module boundary so its inner and outer lint attributes apply.
			self.check_span(cx, spans.inner_span);
		}
	}

	fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
		// Also cover include! sources and files without surviving AST nodes.
		// These remaining files use the crate's lint level.
		for file in cx.sess().source_map().files().iter() {
			self.check_file(cx, file);
		}
	}
}

impl FileLength {
	fn check_span(&mut self, cx: &EarlyContext<'_>, span: Span) {
		if !span.is_dummy() && !span.from_expansion() {
			self.check_file(cx, &cx.sess().source_map().lookup_source_file(span.lo()));
		}
	}

	fn check_file(&mut self, cx: &EarlyContext<'_>, file: &SourceFile) {
		let FileName::Real(name) = &file.name else { return };
		let Some(path) = name.local_path() else { return };
		let Some(source) = &file.src else { return };
		let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
		let relative = cx
			.sess()
			.source_map()
			.working_dir()
			.local_path()
			.and_then(|root| path.strip_prefix(root).ok())
			.unwrap_or(&path);
		if excluded(relative)
			|| std::env::var_os("OUT_DIR").is_some_and(|out| path.starts_with(out))
			|| !self.checked.insert(path)
		{
			return;
		}
		let lines = source.lines().count();
		if lines <= MAX_LINES {
			return;
		}
		let start = file.absolute_position(file.lines()[MAX_LINES]);
		cx.emit_span_lint(
			FILE_LENGTH,
			Span::with_root_ctxt(start, start),
			rustc_errors::DiagDecorator(|diag| {
				diag.primary_message(format!("Rust source file has {lines} lines (max {MAX_LINES})"));
				diag.help("split this file into smaller modules");
			}),
		);
	}
}

fn excluded(path: &Path) -> bool {
	path.extension().is_none_or(|extension| extension != "rs")
		|| path.components().any(|component| {
			matches!(component, Component::Normal(name) if name == "target" || name.as_encoded_bytes().starts_with(b"."))
		})
}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;

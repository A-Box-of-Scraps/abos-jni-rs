#![feature(rustc_private, default_field_values)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rustc_ast::{Attribute, Inline, Item, ItemKind, MetaItem, MetaItemKind, ModKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::{FileName, Span, sym};

dylint_linting::impl_pre_expansion_lint! {
	/// Keeps test implementations in `tests/`, allowing `#[path]` child modules in `src/`.
	pub TESTS_OUTSIDE_SRC,
	Warn,
	"test implementations belong in tests/ rather than src/",
	TestsOutsideSrc::default()
}

#[derive(Default)]
struct TestsOutsideSrc {
	package_roots: HashMap<PathBuf, Option<PathBuf>>,
	allowed_attributes: HashSet<Span>,
	inline_directories: Vec<(Span, PathBuf)>,
	file_has_path: Option<bool>,
}

impl EarlyLintPass for TestsOutsideSrc {
	fn check_attributes(&mut self, _: &EarlyContext<'_>, attrs: &[Attribute]) {
		// Each pre-expansion pass starts at the crate root or a freshly loaded
		// external module. A path-loaded file has no implicit stem directory.
		self.file_has_path
			.get_or_insert_with(|| attrs.iter().any(|attr| attr.has_name(sym::path)));
	}

	fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
		let ItemKind::Mod(_, name, kind) = &item.kind else {
			return;
		};
		let Some((source, root)) = self.source_in_src(cx, item.span) else {
			return;
		};
		let Some(parent) = source.parent() else { return };
		let mut directory = self
			.inline_directories
			.last()
			.map_or_else(|| parent.to_path_buf(), |(_, dir)| dir.clone());
		let path = item
			.attrs
			.iter()
			.find(|attr| attr.has_name(sym::path))
			.and_then(Attribute::value_str);
		if matches!(kind, ModKind::Loaded(_, Inline::Yes, _)) {
			if let Some(path) = path {
				directory.push(path.as_str());
			} else {
				let crate_source = cx
					.sess()
					.local_crate_source_file()
					.and_then(|file| file.local_path()?.canonicalize().ok());
				if self.inline_directories.is_empty()
					&& self.file_has_path != Some(true)
					&& crate_source.as_ref() != Some(&source)
					&& let Some(stem) = source.file_stem()
					&& stem != "mod"
				{
					directory.push(stem);
				}
				directory.push(name.as_str());
			}
			self.inline_directories.push((item.span, directory));
			return;
		}
		let Some(path) = path else { return };
		// Resolve the file, not a textual substring such as "tests/../src/".
		if directory
			.join(path.as_str())
			.canonicalize()
			.is_ok_and(|target| target.starts_with(root.join("tests")))
		{
			self.allowed_attributes.extend(item.attrs.iter().map(|attr| attr.span));
		}
	}

	fn check_item_post(&mut self, _: &EarlyContext<'_>, item: &Item) {
		if self
			.inline_directories
			.last()
			.is_some_and(|(span, _)| *span == item.span)
		{
			self.inline_directories.pop();
		}
	}

	fn check_attribute(&mut self, cx: &EarlyContext<'_>, attr: &Attribute) {
		if self.allowed_attributes.contains(&attr.span)
			|| attr.span.from_expansion()
			|| !attr.meta().is_some_and(|meta| test_attribute(&meta))
			|| self.source_in_src(cx, attr.span).is_none()
		{
			return;
		}
		cx.emit_span_lint(
			TESTS_OUTSIDE_SRC,
			attr.span,
			rustc_errors::DiagDecorator(|diag| {
				diag.primary_message("test implementation in src/");
				diag.help("move this test-only code into tests/; for private access, use a #[cfg(test)] #[path = \"...\"] child module pointing there");
			}),
		);
	}
}

impl TestsOutsideSrc {
	fn source_in_src(&mut self, cx: &EarlyContext<'_>, span: Span) -> Option<(PathBuf, PathBuf)> {
		if span.from_expansion() {
			return None;
		}
		let file = cx.sess().source_map().lookup_source_file(span.lo());
		let FileName::Real(name) = &file.name else { return None };
		let source = name.local_path()?.canonicalize().ok()?;
		// Use the nearest package, so an ancestor directory named src does not
		// accidentally classify integration tests or build scripts as production code.
		let root = self
			.package_roots
			.entry(source.clone())
			.or_insert_with(|| {
				source
					.ancestors()
					.skip(1)
					.find(|dir| dir.join("Cargo.toml").is_file())
					.map(Path::to_path_buf)
			})
			.as_ref()?;
		source.starts_with(root.join("src")).then(|| (source, root.clone()))
	}
}

fn test_attribute(meta: &MetaItem) -> bool {
	if meta
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident.name == sym::test)
	{
		return true;
	}
	let Some(items) = meta.meta_item_list() else {
		return false;
	};
	if meta.has_name(sym::cfg) {
		return items
			.first()
			.and_then(|item| item.meta_item())
			.is_some_and(|cfg| mentions_test(cfg) && cfg_without_test(cfg) == Some(false));
	}
	meta.has_name(sym::cfg_attr)
		&& items
			.iter()
			.skip(1)
			.filter_map(|item| item.meta_item())
			.any(test_attribute)
}

fn mentions_test(meta: &MetaItem) -> bool {
	meta.has_name(sym::test)
		|| meta
			.meta_item_list()
			.is_some_and(|items| items.iter().filter_map(|item| item.meta_item()).any(mentions_test))
}

// Evaluate with test=false, leaving platform/feature predicates unknown.
// In particular, not(test) and any(test, feature = "...") are not test-only.
fn cfg_without_test(meta: &MetaItem) -> Option<bool> {
	if meta.has_name(sym::test) && matches!(meta.kind, MetaItemKind::Word) {
		return Some(false);
	}
	let items = meta.meta_item_list()?;
	if meta.has_name(sym::not) {
		return cfg_without_test(items.first()?.meta_item()?).map(|value| !value);
	}
	let all = meta.has_name(sym::all);
	if !all && !meta.has_name(sym::any) {
		return None;
	}
	let mut unknown = false;
	for item in items {
		match item.meta_item().and_then(cfg_without_test) {
			Some(value) if value != all => return Some(value),
			None => unknown = true,
			_ => {}
		}
	}
	if unknown { None } else { Some(all) }
}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;

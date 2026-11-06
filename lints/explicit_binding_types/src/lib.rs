#![feature(rustc_private, default_field_values)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::intravisit::{InferKind, Visitor};
use rustc_hir::{Expr, ExprKind, LetStmt, LocalSource, PatKind, QPath, TyKind, UnOp};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;

dylint_linting::declare_late_lint! {
	/// Requires a type annotation when a local binding's initializer does not reveal its type.
	pub EXPLICIT_BINDING_TYPES,
	Warn,
	"non-obvious local bindings should have explicit types"
}

impl<'tcx> LateLintPass<'tcx> for ExplicitBindingTypes {
	fn check_local(&mut self, cx: &LateContext<'tcx>, local: &'tcx LetStmt<'tcx>) {
		if !matches!(local.source, LocalSource::Normal)
			|| local.span.from_expansion()
			// Derive macros can give generated locals the span of an original field.
			|| cx.tcx.def_span(local.hir_id.owner.def_id).from_expansion()
			|| !matches!(local.pat.kind, PatKind::Binding(..))
			|| local.ty.is_some_and(explicit_type)
		{
			return;
		}
		let Some(init) = local.init else { return };
		// Closures and opaque return types cannot always be written as concrete local types.
		if matches!(init.kind, ExprKind::Closure(..))
			|| cx.typeck_results().expr_ty(init).is_opaque()
			|| obvious(cx, init)
		{
			return;
		}
		cx.emit_span_lint(
			EXPLICIT_BINDING_TYPES,
			local.pat.span,
			rustc_errors::DiagDecorator(|diag| {
				diag.primary_message("this binding's type is not obvious from its initializer");
				diag.help("add an explicit type annotation to the binding");
			}),
		);
	}
}

fn obvious<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
	match expr.kind {
		ExprKind::Lit(_) | ExprKind::Cast(..) | ExprKind::Type(..) => true,
		ExprKind::Tup(items) | ExprKind::Array(items) => items.iter().all(|item| obvious(cx, item)),
		ExprKind::AddrOf(_, _, inner) | ExprKind::Unary(UnOp::Neg | UnOp::Not, inner) => obvious(cx, inner),
		ExprKind::Struct(ref path, ..) => explicit_adt_path(cx, expr, path),
		ExprKind::Call(callee, _) => {
			let ExprKind::Path(QPath::TypeRelative(owner, method)) = callee.kind else {
				return false;
			};
			let TyKind::Path(ref path) = owner.kind else {
				return false;
			};
			method.ident.as_str() == "new" && explicit_adt_path(cx, expr, path)
		}
		_ => false,
	}
}

fn explicit_adt_path<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>, path: &QPath<'tcx>) -> bool {
	let QPath::Resolved(None, path) = path else {
		return false;
	};
	let ty::Adt(adt, _) = cx.typeck_results().expr_ty(expr).kind() else {
		return false;
	};
	// Check the resolved type, not capitalization: a function called `new` need not return Self.
	if path.res.opt_def_id() != Some(adt.did()) {
		return false;
	}
	let required = cx
		.tcx
		.generics_of(adt.did())
		.own_params
		.iter()
		.filter(|param| {
			matches!(
				param.kind,
				ty::GenericParamDefKind::Type { has_default: false, .. }
					| ty::GenericParamDefKind::Const { has_default: false, .. }
			)
		})
		.count();
	let Some(args) = path.segments.last().and_then(|segment| segment.args) else {
		return required == 0;
	};
	let mut visitor = InferenceVisitor::default();
	visitor.visit_generic_args(args);
	let supplied = args
		.args
		.iter()
		.filter(|arg| matches!(arg, rustc_hir::GenericArg::Type(_) | rustc_hir::GenericArg::Const(_)))
		.count();
	if visitor.found_inference {
		return false;
	}
	supplied >= required
}

#[derive(Default)]
struct InferenceVisitor {
	found_inference: bool,
}
impl<'v> Visitor<'v> for InferenceVisitor {
	fn visit_infer(&mut self, _: rustc_hir::HirId, _: rustc_span::Span, _: InferKind<'v>) {
		self.found_inference = true;
	}
}
fn explicit_type(ty: &rustc_hir::Ty<'_>) -> bool {
	let mut visitor = InferenceVisitor::default();
	visitor.visit_ty_unambig(ty);
	!visitor.found_inference
}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;

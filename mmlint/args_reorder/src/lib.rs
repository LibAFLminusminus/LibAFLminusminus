#![feature(rustc_private)]
#![warn(unused_extern_crates)]

// extern crate rustc_arena;
// extern crate rustc_ast;
// extern crate rustc_ast_pretty;
// extern crate rustc_data_structures;
// extern crate rustc_errors;
extern crate rustc_hir;
// extern crate rustc_hir_pretty;
// extern crate rustc_index;
// extern crate rustc_infer;
// extern crate rustc_lexer;
// extern crate rustc_middle;
// extern crate rustc_mir_dataflow;
// extern crate rustc_parse;
extern crate rustc_span;
// extern crate rustc_target;
// extern crate rustc_trait_selection;

use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_hir::{intravisit::FnKind, FnDecl, Body, def_id::LocalDefId, PatKind,};
use rustc_span::{Span, Symbol};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Check if the arguments are correctly ordered
    /// A few exceptions are
    /// - closures (we don't care abt it)
    /// - arguments starting with _
    ///
    /// ### Why is this bad?
    /// Not bad, just for style.
    ///
    /// ### Example
    ///
    /// ```rust
    /// // this is warned because a comes before b in latin alphabet
    /// pub fn foo(b: u8, a: u8) {}
    ///
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// pub fn foo(a: u8, b: u8) {}
    /// ```
    pub ARGS_REORDER,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for ArgsReorder {
    // A list of things you might check can be found here:
    // https://doc.rust-lang.org/stable/nightly-rustc/rustc_lint/trait.LateLintPass.html
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        _: LocalDefId,
    ) {
        match kind {
            FnKind::Closure => {
                return; // ignore closures
            }
            _ => (),
        }

        let mut names = vec![];

        for param in body.params {
            if let PatKind::Binding(_, _, ident, _) = param.pat.kind {
                if ident.name.as_str().starts_with('_') { // ignore names starting with _
                    continue;
                }

                names.push((ident.name, param.span));
            }
        }

        check_args(cx, &names)
    }
}

fn check_args(cx: &LateContext<'_>, params: &[(Symbol, Span)]) {
    let first = params.first();
    let last = params.last();
    if let (Some((_, first_span)), Some((_, last_span))) = (first, last) {
        let names: Vec<String> = params.iter().map(|(s, _)| s.to_string()).collect::<Vec<String>>();
        if !names.is_sorted() {
            let merged_span = first_span.to(*last_span);

            cx.span_lint(ARGS_REORDER, merged_span, |diag| {
                diag.primary_message("function arguments are not sorted in a alphabetical order");
            });
        }
    }
}


#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

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

use rustc_lint::{LateLintPass, LateContext, LintContext};
use rustc_hir::{Generics, GenericParamKind, ParamName};
use rustc_span::{Symbol, Span};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Sort the type generics alphabetically
    /// ### Why is this bad?
    /// Not bad, but sorted looks more beautiful no?
    /// ### Known problems
    /// No problem
    /// 
    ///
    /// ### Example
    ///
    /// ```rust
    /// // this will be warned cuz Z comes in A in latin alphabet.
    /// pub trait LOL<Z, A> {} 
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// pub trait LOL<A, Z> {} 
    /// ```
    pub GENERIC_REORDER,
    Warn,
    "description goes here"
}

impl<'tcx> LateLintPass<'tcx> for GenericReorder {
    // A list of things you might check can be found here:
    // https://doc.rust-lang.org/stable/nightly-rustc/rustc_lint/trait.LateLintPass.html
    fn check_generics(&mut self, cx: &LateContext<'tcx>, generics: &'tcx Generics<'_>) {
        let params = generics.params;
        let mut names = vec![];
        for param in params {
            match param.kind {
                GenericParamKind::Lifetime { .. } | GenericParamKind::Const { .. } => {
                    continue;
                }, // don't care abt lifetime or const
                GenericParamKind::Type { .. } => {
                    let ParamName::Plain(ident) = param.name else { // Plain is the user given name
                        continue;
                    };
                    names.push((ident.name, param.span)); // i need span cuz later it's used for span_lint
                },
            }
        }
        check_generics(cx, &names);
        // println!("{:?}", names);
    }
}

fn check_generics(cx: &LateContext<'_>, params: &[(Symbol, Span)]) {
    let first = params.first();
    let last = params.last();

    if let (Some((_, first_span)), Some((_, last_span))) = (first, last) {
        let names: Vec<String> = params.iter().map(|(s, _)| s.to_string()).collect::<Vec<String>>();
        if !names.is_sorted() {
            let merged_span = first_span.to(*last_span);

            cx.span_lint(GENERIC_REORDER, merged_span, |diag| {
                diag.primary_message("generic parameters are not sorted in a alphabetical order");
            });
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

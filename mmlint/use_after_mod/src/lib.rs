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
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

dylint_linting::impl_late_lint! {
    /// ### What it does
    /// It forces you to put `pub use a::*` right after `pub mod a`;
    ///
    /// ### Why is this bad?
    /// Not bad but it's a problem of a style
    ///
    /// ### Example
    ///
    /// ```rust
    /// pub mod a;
    /// pub static a: u8 = 0; // no. this guy should come between `pub mod` and `pub use`
    /// pub use a::*
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// pub mod a;
    /// pub use a::*
    ///
    /// // whatever you want continues
    /// ```
    pub USE_AFTER_MOD,
    Warn,
    "description goes here",
    UseAfterMod::default()
}

#[derive(Debug, Default)]
pub struct UseAfterMod {
    item: Option<ModUseItem>,
}

#[derive(Debug, Copy, Clone)]
enum ModUseItem {
    Mod(Span),
    Use,
    Others(Span),
}

impl<'tcx> LateLintPass<'tcx> for UseAfterMod {
    // A list of things you might check can be found here:
    // https://doc.rust-lang.org/stable/nightly-rustc/rustc_lint/trait.LateLintPass.html
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let item: ModUseItem = match item.kind {
            ItemKind::Mod(_, _) => ModUseItem::Mod(item.span),
            ItemKind::Use(_, _) => ModUseItem::Use,
            _ => ModUseItem::Others(item.span),
        };

        check_mod_use(cx, self.item, item);
        self.item = Some(item);
    }
}

fn check_mod_use(cx: &LateContext<'_>, prev: Option<ModUseItem>, cur: ModUseItem) {
    if let Some(prev) = prev {
        match (prev, cur) {
            (ModUseItem::Mod(span1), ModUseItem::Others(span2)) => {
                let merged_span = span1.to(span2);
                cx.span_lint(USE_AFTER_MOD, merged_span, |diag| {
                    diag.primary_message("`use` should immediately follow `mod`");
                });
            }
            _ => (), // do nothing
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

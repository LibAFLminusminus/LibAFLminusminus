//! Lint: generic parameters must be declared in alphabetic order, including `where` clauses.

use std::{
    fs::read_to_string,
    io,
    path::{Path, PathBuf},
};

use syn::{GenericParam, Generics, ImplItem, Item, TraitItem, WherePredicate, spanned::Spanned};

use super::{Label, render_diagnostic};

struct Ctx<'a> {
    path: &'a str,
    src: &'a str,
    src_lines: Vec<&'a str>,
    violations: &'a mut Vec<String>,
}

pub async fn run_generics_order_check(rs_file_path: PathBuf, verbose: bool) -> io::Result<()> {
    if verbose {
        println!(
            "[*] Checking generics order {}...",
            rs_file_path.as_path().display()
        );
    }

    let src = read_to_string(&rs_file_path)?;
    let violations = check_generics_order(&rs_file_path, &src);

    if violations.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(violations.join("\n\n")))
    }
}

fn check_generics_order(rs_file_path: &Path, src: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(src) else {
        return Vec::new();
    };

    let path = rs_file_path.display().to_string();
    let src_lines: Vec<&str> = src.lines().collect();
    let mut violations: Vec<String> = Vec::new();

    let mut ctx = Ctx {
        path: &path,
        src,
        src_lines,
        violations: &mut violations,
    };

    for item in &file.items {
        visit_item(item, &mut ctx);
    }

    violations
}

fn visit_item(item: &Item, ctx: &mut Ctx<'_>) {
    match item {
        Item::Fn(f) => check_generics(&f.sig.generics, ctx),
        Item::Struct(s) => check_generics(&s.generics, ctx),
        Item::Enum(e) => check_generics(&e.generics, ctx),
        Item::Union(u) => check_generics(&u.generics, ctx),
        Item::Type(t) => check_generics(&t.generics, ctx),
        Item::Trait(t) => {
            check_generics(&t.generics, ctx);
            for ti in &t.items {
                visit_trait_item(ti, ctx);
            }
        }
        Item::Impl(i) => {
            check_generics(&i.generics, ctx);
            for ii in &i.items {
                visit_impl_item(ii, ctx);
            }
        }
        Item::Mod(m) => {
            if let Some((_, items)) = &m.content {
                for inner in items {
                    visit_item(inner, ctx);
                }
            }
        }
        _ => {}
    }
}

fn visit_impl_item(item: &ImplItem, ctx: &mut Ctx<'_>) {
    match item {
        ImplItem::Fn(f) => check_generics(&f.sig.generics, ctx),
        ImplItem::Type(t) => check_generics(&t.generics, ctx),
        _ => {}
    }
}

fn visit_trait_item(item: &TraitItem, ctx: &mut Ctx<'_>) {
    match item {
        TraitItem::Fn(f) => check_generics(&f.sig.generics, ctx),
        TraitItem::Type(t) => check_generics(&t.generics, ctx),
        _ => {}
    }
}

struct ParamInfo {
    name: String,
    kind: &'static str,
    span: proc_macro2::Span,
    display: String,
}

fn param_info(param: &GenericParam) -> ParamInfo {
    match param {
        GenericParam::Lifetime(lp) => {
            let name = lp.lifetime.ident.to_string();
            ParamInfo {
                display: format!("'{name}"),
                name,
                kind: "lifetime",
                span: lp.lifetime.span(),
            }
        }
        GenericParam::Type(tp) => {
            let name = tp.ident.to_string();
            ParamInfo {
                display: name.clone(),
                name,
                kind: "type",
                span: tp.ident.span(),
            }
        }
        GenericParam::Const(cp) => {
            let name = cp.ident.to_string();
            ParamInfo {
                display: name.clone(),
                name,
                kind: "const",
                span: cp.ident.span(),
            }
        }
    }
}

fn check_generics(generics: &Generics, ctx: &mut Ctx<'_>) {
    let mut prev_lifetime: Option<ParamInfo> = None;
    let mut prev_type: Option<ParamInfo> = None;
    let mut prev_const: Option<ParamInfo> = None;

    for param in &generics.params {
        let info = param_info(param);

        let prev = match param {
            GenericParam::Lifetime(_) => &mut prev_lifetime,
            GenericParam::Type(_) => &mut prev_type,
            GenericParam::Const(_) => &mut prev_const,
        };

        if let Some(prev_info) = prev.as_ref()
            && info.name.as_str() < prev_info.name.as_str()
        {
            emit_ordering_violation(
                ctx,
                "generic parameter",
                info.kind,
                &info.display,
                info.span,
                &prev_info.display,
                prev_info.span,
            );
        }

        *prev = Some(info);
    }

    if let Some(where_clause) = &generics.where_clause {
        check_where_clause(where_clause, ctx);
    }
}

fn check_where_clause(where_clause: &syn::WhereClause, ctx: &mut Ctx<'_>) {
    let mut prev_lifetime: Option<ParamInfo> = None;
    let mut prev_type: Option<ParamInfo> = None;

    for pred in &where_clause.predicates {
        let info = match pred {
            WherePredicate::Lifetime(pl) => {
                let name = pl.lifetime.ident.to_string();
                ParamInfo {
                    display: format!("'{name}"),
                    name,
                    kind: "lifetime",
                    span: pl.lifetime.span(),
                }
            }
            WherePredicate::Type(pt) => {
                let span = pt.bounded_ty.span();
                let text = span_source_text(span, &ctx.src_lines);
                ParamInfo {
                    name: text.clone(),
                    kind: "type",
                    span,
                    display: text,
                }
            }
            _ => continue,
        };

        let prev = match pred {
            WherePredicate::Lifetime(_) => &mut prev_lifetime,
            WherePredicate::Type(_) => &mut prev_type,
            _ => unreachable!(),
        };

        if let Some(prev_info) = prev.as_ref()
            && info.name.as_str() < prev_info.name.as_str()
        {
            emit_ordering_violation(
                ctx,
                "`where`-clause predicate",
                info.kind,
                &info.display,
                info.span,
                &prev_info.display,
                prev_info.span,
            );
        }

        *prev = Some(info);
    }
}

#[expect(clippy::too_many_arguments)]
fn emit_ordering_violation(
    ctx: &mut Ctx<'_>,
    category: &str,
    kind: &str,
    display: &str,
    span: proc_macro2::Span,
    prev_display: &str,
    prev_span: proc_macro2::Span,
) {
    let loc = span.start();
    let prev_loc = prev_span.start();

    let primary_text = format!("`{display}` should come before `{prev_display}`");
    let secondary_text = format!("`{prev_display}` declared here");
    let help = format!("reorder so {kind} {category}s appear alphabetically");

    ctx.violations.push(render_diagnostic(
        "generics-order",
        &format!("{kind} {category} `{display}` is out of alphabetic order"),
        ctx.path,
        ctx.src,
        Label {
            line: loc.line,
            col: loc.column + 1,
            span_len: display.len(),
            text: &primary_text,
            primary: true,
        },
        Some(Label {
            line: prev_loc.line,
            col: prev_loc.column + 1,
            span_len: prev_display.len(),
            text: &secondary_text,
            primary: false,
        }),
        &help,
    ));
}

fn span_source_text(span: proc_macro2::Span, src_lines: &[&str]) -> String {
    let start = span.start();
    let end = span.end();
    if start.line == 0 || end.line == 0 {
        return String::new();
    }

    if start.line == end.line {
        let line = src_lines.get(start.line - 1).copied().unwrap_or("");
        let s = start.column.min(line.len());
        let e = end.column.min(line.len());
        return line.get(s..e).unwrap_or("").to_string();
    }

    let mut out = String::new();
    for line_no in start.line..=end.line {
        let line = src_lines.get(line_no - 1).copied().unwrap_or("");
        if line_no == start.line {
            let s = start.column.min(line.len());
            out.push_str(line.get(s..).unwrap_or(""));
        } else if line_no == end.line {
            let e = end.column.min(line.len());
            out.push_str(line.get(..e).unwrap_or(""));
        } else {
            out.push_str(line);
        }
        out.push(' ');
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::check_generics_order;

    fn path() -> PathBuf {
        PathBuf::from("test.rs")
    }

    #[test]
    fn accepts_sorted_type_params() {
        let src = "fn f<A, AB, C, Z>() {}\n";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn rejects_unsorted_type_params() {
        let src = "fn f<AB, A, C, Z>() {}\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("generics-order"));
        assert!(v[0].contains("`A`"));
        assert!(v[0].contains("`AB`"));
    }

    #[test]
    fn reports_every_out_of_order_pair() {
        let src = "fn f<B, A, D, C>() {}\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn checks_lifetimes_within_their_own_kind() {
        let src = "fn f<'b, 'a, T>() {}\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("lifetime"));
        assert!(v[0].contains("'a"));
        assert!(v[0].contains("'b"));
    }

    #[test]
    fn lifetimes_do_not_compare_against_types() {
        let src = "fn f<'z, A>() {}\n";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn rejects_unsorted_consts() {
        let src = "fn f<const N: usize, const M: usize>() {}\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("const"));
    }

    #[test]
    fn accepts_sorted_consts() {
        let src = "fn f<const M: usize, const N: usize>() {}\n";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn checks_struct_generics() {
        let src = "struct S<Z, A>(Z, A);\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn checks_enum_generics() {
        let src = "enum E<Z, A> { V(Z, A) }\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn checks_trait_generics() {
        let src = "trait Tr<Z, A> {}\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn checks_impl_generics() {
        let src = "struct S<A, B>(A, B); impl<Z, A> S<A, Z> {}\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn checks_type_alias_generics() {
        let src = "type T<Z, A> = (Z, A);\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn recurses_into_inline_modules() {
        let src = "mod inner { fn f<Z, A>() {} }\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn recurses_into_impl_methods() {
        let src = "struct S; impl S { fn f<Z, A>() {} }\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn recurses_into_trait_methods() {
        let src = "trait Tr { fn f<Z, A>() {} }\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn mixed_sorted_all_kinds_accepted() {
        let src = "fn f<'a, 'b, T, U, const M: usize, const N: usize>() {}\n";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn example_from_user_request_valid() {
        let src = "struct S<A, AB, C, Z>(A, AB, C, Z);\n";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn example_from_user_request_invalid() {
        let src = "struct S<AB, A, C, Z>(AB, A, C, Z);\n";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn where_clause_sorted_ok() {
        let src = r"
struct MyStruct<A, AB, C>(A, AB, C);
impl<A, AB, C> MyStruct<A, AB, C>
where
    A: Clone,
    AB: Clone,
    C: Clone,
{}
";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn where_clause_unsorted_flagged() {
        let src = r"
struct MyStruct<A, AB, C>(A, AB, C);
impl<A, AB, C> MyStruct<A, AB, C>
where
    A: Clone,
    C: Clone,
    AB: Clone,
{}
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("where"));
        assert!(v[0].contains("AB"));
        assert!(v[0].contains("C"));
    }

    #[test]
    fn where_clause_lifetimes_checked_within_own_kind() {
        let src = r"
fn f<'a, 'b, T>(_: &'a T, _: &'b T)
where
    'b: 'a,
    'a: 'a,
    T: Clone,
{}
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("lifetime"));
    }

    #[test]
    fn where_clause_on_fn_checked() {
        let src = r"
fn f<A, B>() where B: Clone, A: Clone {}
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn where_clause_on_trait_checked() {
        let src = r"
trait Tr<A, B> where B: Clone, A: Clone {}
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn where_clause_on_type_alias_checked() {
        let src = r"
type T<A, B> where B: Clone, A: Clone = (A, B);
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn where_clause_multiple_violations_accumulate() {
        let src = r"
fn f<A, B, C, D>() where D: Clone, A: Clone, C: Clone, B: Clone {}
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn where_clause_empty_is_fine() {
        let src = r"
fn f<A, B>() where {}
";
        assert!(check_generics_order(&path(), src).is_empty());
    }

    #[test]
    fn generics_and_where_clause_both_checked() {
        let src = r"
fn f<B, A>() where B: Clone, A: Clone {}
";
        let v = check_generics_order(&path(), src);
        assert_eq!(v.len(), 2);
    }
}

//! Item-ordering lints.

use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    io,
    path::{Path, PathBuf},
};

use syn::{
    ForeignItem,
    Item::{
        self, Const, Enum, ExternCrate, Fn, ForeignMod, Impl, Macro, Mod, Static, Struct, Trait,
        TraitAlias, Type, Union, Use,
    },
    spanned::Spanned,
};

use super::{Label, render_diagnostic, underline_len};

fn is_pub_use(u: &syn::ItemUse) -> bool {
    !matches!(u.vis, syn::Visibility::Inherited)
}

fn is_pub_mod(m: &syn::ItemMod) -> bool {
    !matches!(m.vis, syn::Visibility::Inherited)
}

fn item_order_rank(item: &Item) -> u8 {
    match item {
        Use(u) if !is_pub_use(u) => 0,
        Mod(m) if !is_pub_mod(m) => 1,
        Use(_) | ExternCrate(_) | Mod(_) => 2,
        Const(_) => 3,
        Static(_) => 4,
        Type(_) => 5,
        Trait(_) | TraitAlias(_) => 6,
        Struct(_) | Enum(_) | Union(_) => 7,
        Impl(_) | Fn(_) => 8,
        _ => 8,
    }
}

fn item_kind_name(item: &Item) -> &'static str {
    match item {
        Use(u) if is_pub_use(u) => "`pub use`",
        Use(_) => "`use`",
        Mod(m) if is_pub_mod(m) => "`pub mod`",
        Mod(_) => "`mod`",
        ExternCrate(_) => "`extern crate`",
        Trait(_) => "`trait`",
        TraitAlias(_) => "trait alias",
        Struct(_) => "`struct`",
        Enum(_) => "`enum`",
        Union(_) => "`union`",
        Type(_) => "type alias",
        Impl(_) => "`impl` block",
        Fn(_) => "`fn`",
        Const(_) => "`const`",
        Static(_) => "`static`",
        Macro(_) => "macro invocation",
        ForeignMod(_) => "`extern {}` block",
        _ => "item",
    }
}

fn foreign_item_order_rank(item: &ForeignItem) -> u8 {
    match item {
        ForeignItem::Static(_) => 4,
        ForeignItem::Type(_) => 5,
        ForeignItem::Fn(_) => 8,
        _ => 8,
    }
}

fn foreign_item_kind_name(item: &ForeignItem) -> &'static str {
    match item {
        ForeignItem::Static(_) => "`static`",
        ForeignItem::Type(_) => "type alias",
        ForeignItem::Fn(_) => "`fn`",
        ForeignItem::Macro(_) => "macro invocation",
        _ => "foreign item",
    }
}

fn item_header_span(item: &Item) -> proc_macro2::Span {
    match item {
        Use(u) => vis_span_or(&u.vis, u.use_token.span()),
        Mod(m) => vis_span_or(&m.vis, m.mod_token.span()),
        Fn(f) => vis_span_or(&f.vis, f.sig.fn_token.span()),
        Struct(s) => vis_span_or(&s.vis, s.struct_token.span()),
        Enum(e) => vis_span_or(&e.vis, e.enum_token.span()),
        Union(u) => vis_span_or(&u.vis, u.union_token.span()),
        Trait(t) => vis_span_or(&t.vis, t.trait_token.span()),
        TraitAlias(t) => vis_span_or(&t.vis, t.trait_token.span()),
        Const(c) => vis_span_or(&c.vis, c.const_token.span()),
        Static(s) => vis_span_or(&s.vis, s.static_token.span()),
        Type(t) => vis_span_or(&t.vis, t.type_token.span()),
        Impl(i) => match &i.unsafety {
            Some(u) => u.span(),
            None => i.impl_token.span(),
        },
        ExternCrate(e) => vis_span_or(&e.vis, e.extern_token.span()),
        ForeignMod(f) => f.abi.extern_token.span(),
        Macro(m) => m.mac.path.span(),
        _ => item.span(),
    }
}

fn foreign_item_header_span(item: &ForeignItem) -> proc_macro2::Span {
    match item {
        ForeignItem::Fn(f) => vis_span_or(&f.vis, f.sig.fn_token.span()),
        ForeignItem::Static(s) => vis_span_or(&s.vis, s.static_token.span()),
        ForeignItem::Type(t) => vis_span_or(&t.vis, t.type_token.span()),
        ForeignItem::Macro(m) => m.mac.path.span(),
        _ => item.span(),
    }
}

fn vis_span_or(vis: &syn::Visibility, fallback: proc_macro2::Span) -> proc_macro2::Span {
    match vis {
        syn::Visibility::Inherited => fallback,
        _ => vis.span(),
    }
}

fn use_tree_first_ident(tree: &syn::UseTree) -> Option<String> {
    match tree {
        syn::UseTree::Path(p) => Some(p.ident.to_string()),
        syn::UseTree::Name(n) => Some(n.ident.to_string()),
        syn::UseTree::Rename(r) => Some(r.ident.to_string()),
        syn::UseTree::Glob(_) | syn::UseTree::Group(_) => None,
    }
}

struct Entry {
    rank: u8,
    kind: &'static str,
    line: usize,
    col: usize,
}

pub async fn run_item_order_check(rs_file_path: PathBuf, verbose: bool) -> io::Result<()> {
    if verbose {
        println!(
            "[*] Checking item order {}...",
            rs_file_path.as_path().display()
        );
    }

    let src = read_to_string(&rs_file_path)?;
    let violations = check_item_order(&rs_file_path, &src);

    if violations.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(violations.join("\n\n")))
    }
}

fn check_item_order(rs_file_path: &Path, src: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(src) else {
        return Vec::new();
    };

    let src_lines: Vec<&str> = src.lines().collect();

    let mut exempt_use_indices: HashSet<usize> = HashSet::new();
    for (i, item) in file.items.iter().enumerate() {
        if let Mod(m) = item
            && m.content.is_none()
            && let Some(Use(u)) = file.items.get(i + 1)
            && is_pub_mod(m) == is_pub_use(u)
            && let Some(seg) = use_tree_first_ident(&u.tree)
            && seg == m.ident.to_string()
        {
            exempt_use_indices.insert(i + 1);
        }
    }

    let mut entries: Vec<Entry> = Vec::new();
    for (idx, item) in file.items.iter().enumerate() {
        if let Mod(m) = item
            && m.content.is_some()
        {
            continue;
        }

        if exempt_use_indices.contains(&idx) {
            continue;
        }

        if let ForeignMod(fm) = item {
            for fi in &fm.items {
                let loc = foreign_item_header_span(fi).start();
                entries.push(Entry {
                    rank: foreign_item_order_rank(fi),
                    kind: foreign_item_kind_name(fi),
                    line: loc.line,
                    col: loc.column + 1,
                });
            }
            continue;
        }

        let loc = item_header_span(item).start();
        entries.push(Entry {
            rank: item_order_rank(item),
            kind: item_kind_name(item),
            line: loc.line,
            col: loc.column + 1,
        });
    }

    let mut max_rank: u8 = 0;
    let mut max_rank_kind: &'static str = "";
    let mut max_rank_line: usize = 0;
    let mut max_rank_col: usize = 0;
    let mut violations: Vec<String> = Vec::new();

    for entry in &entries {
        if entry.rank < max_rank {
            let path = rs_file_path.display().to_string();
            let cur_src = src_lines.get(entry.line - 1).copied().unwrap_or("");
            let prev_src = src_lines.get(max_rank_line - 1).copied().unwrap_or("");

            let primary_label = format!("{} must come before any {max_rank_kind}", entry.kind);
            let secondary_label = format!("{max_rank_kind} here");
            let help = format!("move this {} above line {max_rank_line}", entry.kind);

            violations.push(render_diagnostic(
                "item-order",
                &format!("{} cannot appear after {max_rank_kind}", entry.kind),
                &path,
                src,
                Label {
                    line: entry.line,
                    col: entry.col,
                    span_len: underline_len(cur_src, entry.col),
                    text: &primary_label,
                    primary: true,
                },
                Some(Label {
                    line: max_rank_line,
                    col: max_rank_col,
                    span_len: underline_len(prev_src, max_rank_col),
                    text: &secondary_label,
                    primary: false,
                }),
                &help,
            ));
            continue;
        }

        if entry.rank > max_rank {
            max_rank = entry.rank;
            max_rank_kind = entry.kind;
            max_rank_line = entry.line;
            max_rank_col = entry.col;
        }
    }

    violations
}

pub async fn run_mod_use_adjacency_check(rs_file_path: PathBuf, verbose: bool) -> io::Result<()> {
    if verbose {
        println!(
            "[*] Checking mod/use adjacency {}...",
            rs_file_path.as_path().display()
        );
    }

    let src = read_to_string(&rs_file_path)?;
    let violations = check_mod_use_adjacency(&rs_file_path, &src);

    if violations.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(violations.join("\n\n")))
    }
}

fn check_mod_use_adjacency(rs_file_path: &Path, src: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(src) else {
        return Vec::new();
    };

    let mut mod_positions: HashMap<String, (usize, bool)> = HashMap::new();
    for (i, item) in file.items.iter().enumerate() {
        if let Mod(m) = item
            && m.content.is_none()
        {
            mod_positions.insert(m.ident.to_string(), (i, is_pub_mod(m)));
        }
    }

    let mut first_use_for_mod: HashMap<String, usize> = HashMap::new();
    for (i, item) in file.items.iter().enumerate() {
        if let Use(u) = item
            && let Some(seg) = use_tree_first_ident(&u.tree)
            && let Some((_, mod_is_pub)) = mod_positions.get(&seg)
            && is_pub_use(u) == *mod_is_pub
        {
            first_use_for_mod.entry(seg).or_insert(i);
        }
    }

    let src_lines: Vec<&str> = src.lines().collect();

    let mut pairs: Vec<(&String, &usize)> = first_use_for_mod.iter().collect();
    pairs.sort_by_key(|(_, use_idx)| **use_idx);

    let mut violations: Vec<String> = Vec::new();

    for (name, use_idx) in pairs {
        let (mod_idx, _) = mod_positions[name];
        if *use_idx != mod_idx + 1 {
            let mod_loc = item_header_span(&file.items[mod_idx]).start();
            let mod_col = mod_loc.column + 1;
            let use_loc = item_header_span(&file.items[*use_idx]).start();
            let use_col = use_loc.column + 1;

            let path = rs_file_path.display().to_string();
            let mod_src = src_lines.get(mod_loc.line - 1).copied().unwrap_or("");
            let use_src = src_lines.get(use_loc.line - 1).copied().unwrap_or("");

            let primary_label = format!(
                "expected on line {} (right after `mod {name};`)",
                mod_loc.line + 1
            );
            let secondary_label = format!("`mod {name};` declared here");
            let help = format!(
                "move this `use` so it is the first item following `mod {name};` on line {}",
                mod_loc.line
            );

            violations.push(render_diagnostic(
                "mod-use-adjacency",
                &format!("`use {name}::...;` must immediately follow `mod {name};`"),
                &path,
                src,
                Label {
                    line: use_loc.line,
                    col: use_col,
                    span_len: underline_len(use_src, use_col),
                    text: &primary_label,
                    primary: true,
                },
                Some(Label {
                    line: mod_loc.line,
                    col: mod_col,
                    span_len: underline_len(mod_src, mod_col),
                    text: &secondary_label,
                    primary: false,
                }),
                &help,
            ));
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{check_item_order, check_mod_use_adjacency};

    fn path() -> PathBuf {
        PathBuf::from("test.rs")
    }

    #[test]
    fn item_order_accepts_canonical_layout() {
        let src = r"
use std::fmt;

const C: u32 = 1;
static S: u32 = 2;
type T = u32;

trait Tr {}
struct St;
enum En { A }

impl St {}
fn f() {}
";
        assert!(check_item_order(&path(), src).is_empty());
    }

    #[test]
    fn item_order_rejects_use_after_fn() {
        let src = r"
fn f() {}
use std::fmt;
";
        let v = check_item_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("item-order"));
        assert!(v[0].contains("`use`"));
    }

    #[test]
    fn item_order_rejects_struct_after_impl() {
        let src = r"
struct First;
impl First {}
struct Second;
";
        let v = check_item_order(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("`struct`"));
    }

    #[test]
    fn item_order_allows_pub_use_after_pub_mod() {
        let src = r"
pub mod inner;
pub use inner::*;

fn after() {}
";
        assert!(check_item_order(&path(), src).is_empty());
    }

    #[test]
    fn item_order_allows_private_mod_use_pair() {
        let src = r"
mod inner;
use inner::Thing;

fn after() {}
";
        assert!(check_item_order(&path(), src).is_empty());
    }

    #[test]
    fn item_order_skips_inline_modules() {
        let src = r"
use std::fmt;

mod inline {
    fn f() {}
    use std::io;
}

fn top() {}
";
        assert!(check_item_order(&path(), src).is_empty());
    }

    #[test]
    fn item_order_multiple_violations_accumulate() {
        let src = r"
fn first() {}
use std::fmt;
struct After;
";
        let v = check_item_order(&path(), src);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn mod_use_adjacency_accepts_adjacent_pair() {
        let src = r"
mod inner;
use inner::Thing;
";
        assert!(check_mod_use_adjacency(&path(), src).is_empty());
    }

    #[test]
    fn mod_use_adjacency_rejects_gap() {
        let src = r"
mod inner;
mod other;
use inner::Thing;
";
        let v = check_mod_use_adjacency(&path(), src);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("mod-use-adjacency"));
        assert!(v[0].contains("inner"));
    }

    #[test]
    fn mod_use_adjacency_visibility_must_match() {
        let src = r"
mod inner;
pub use inner::Thing;
";
        assert!(check_mod_use_adjacency(&path(), src).is_empty());
    }

    #[test]
    fn mod_use_adjacency_pub_pair_accepted() {
        let src = r"
pub mod inner;
pub use inner::*;
";
        assert!(check_mod_use_adjacency(&path(), src).is_empty());
    }

    #[test]
    fn mod_use_adjacency_reports_only_first_use() {
        let src = r"
mod inner;
fn gap() {}
use inner::A;
use inner::B;
";
        let v = check_mod_use_adjacency(&path(), src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn mod_use_adjacency_handles_unknown_mod() {
        let src = r"
use std::fmt;
mod inner;
";
        assert!(check_mod_use_adjacency(&path(), src).is_empty());
    }
}

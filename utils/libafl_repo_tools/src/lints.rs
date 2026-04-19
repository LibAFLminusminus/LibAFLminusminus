//! Custom source-level lints for the `LibAFL` repository.
//!
//! Each lint produces a rustc-style diagnostic on violation.

use colored::Colorize;
use std::{collections::HashMap, fs::read_to_string, io, path::PathBuf};
use syn::ForeignItem;
use syn::Item::{
    self, Const, Enum, ExternCrate, Fn, ForeignMod, Impl, Macro, Mod, Static, Struct, Trait,
    TraitAlias, Type, Union, Use,
};
use syn::spanned::Spanned;

struct Label<'a> {
    line: usize,
    col: usize,
    span_len: usize,
    text: &'a str,
    primary: bool,
}

/// Render a rustc-style diagnostic.
fn render_diagnostic(
    code: &str,
    message: &str,
    path: &str,
    src: &str,
    primary: Label<'_>,
    secondary: Option<Label<'_>>,
    help: &str,
) -> String {
    let src_lines: Vec<&str> = src.lines().collect();
    let all_labels: Vec<&Label> = secondary.iter().chain(std::iter::once(&primary)).collect();
    let gutter_w = all_labels
        .iter()
        .map(|l| l.line.to_string().len())
        .max()
        .unwrap_or(1);
    let gutter_pad = " ".repeat(gutter_w);
    let sep = "|".blue().bold();

    let mut out = String::new();

    out += &format!(
        "{}{} {}\n",
        format!("error[{code}]").red().bold(),
        ":".bold(),
        message.bold(),
    );

    out += &format!(
        "{gutter_pad}{arrow} {path}:{line}:{col}\n",
        arrow = "-->".blue().bold(),
        line = primary.line,
        col = primary.col,
    );

    out += &format!("{gutter_pad} {sep}\n");

    let mut labels_sorted: Vec<&Label> = all_labels.clone();
    labels_sorted.sort_by_key(|l| l.line);

    let mut prev_line: Option<usize> = None;
    for label in &labels_sorted {
        if let Some(prev) = prev_line
            && label.line > prev + 1
        {
            out += &format!("{}\n", "...".blue().bold());
        }

        let source_line = src_lines.get(label.line - 1).copied().unwrap_or("");
        let ln = format!("{:>w$}", label.line, w = gutter_w).blue().bold();
        out += &format!("{ln} {sep} {source_line}\n");

        let marker = if label.primary { "^" } else { "-" };
        let marker_line = marker.repeat(label.span_len.max(1));
        let (marker_colored, text_colored) = if label.primary {
            (marker_line.red().bold(), label.text.red().bold())
        } else {
            (marker_line.blue().bold(), label.text.blue().bold())
        };
        out += &format!(
            "{gutter_pad} {sep} {offset}{marker_colored} {text_colored}\n",
            offset = " ".repeat(label.col.saturating_sub(1)),
        );

        prev_line = Some(label.line);
    }

    out += &format!("{gutter_pad} {sep}\n");
    out += &format!(
        "{gutter_pad} {eq} {help_label}: {help}",
        eq = "=".blue().bold(),
        help_label = "help".bold(),
    );

    out
}

fn underline_len(src_line: &str, col: usize) -> usize {
    let remaining = src_line.get(col.saturating_sub(1)..).unwrap_or("");
    let stop = remaining.find(['{', ';']).unwrap_or(remaining.len());
    remaining[..stop].trim_end().len().max(1)
}

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

fn use_tree_first_ident(tree: &syn::UseTree) -> Option<String> {
    match tree {
        syn::UseTree::Path(p) => Some(p.ident.to_string()),
        syn::UseTree::Name(n) => Some(n.ident.to_string()),
        syn::UseTree::Rename(r) => Some(r.ident.to_string()),
        syn::UseTree::Glob(_) | syn::UseTree::Group(_) => None,
    }
}

pub async fn run_file_lints(rs_file_path: PathBuf, verbose: bool) -> io::Result<()> {
    let mut errors: Vec<String> = Vec::new();

    if let Err(e) = run_item_order_check(rs_file_path.clone(), verbose).await {
        errors.push(e.to_string());
    }
    if let Err(e) = run_mod_use_adjacency_check(rs_file_path, verbose).await {
        errors.push(e.to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(errors.join("\n\n")))
    }
}

pub async fn run_item_order_check(rs_file_path: PathBuf, verbose: bool) -> io::Result<()> {
    if verbose {
        println!(
            "[*] Checking item order {}...",
            rs_file_path.as_path().display()
        );
    }

    let src = read_to_string(&rs_file_path)?;

    let Ok(file) = syn::parse_file(&src) else {
        if verbose {
            println!(
                "[*] \tSkipping unparseable file {}",
                rs_file_path.as_path().display()
            );
        }
        return Ok(());
    };

    let src_lines: Vec<&str> = src.lines().collect();

    struct Entry {
        rank: u8,
        kind: &'static str,
        line: usize,
        col: usize,
    }

    let mut entries: Vec<Entry> = Vec::new();
    for item in &file.items {
        if let Mod(m) = item
            && m.content.is_some()
        {
            continue;
        }

        if let ForeignMod(fm) = item {
            for fi in &fm.items {
                let loc = fi.span().start();
                entries.push(Entry {
                    rank: foreign_item_order_rank(fi),
                    kind: foreign_item_kind_name(fi),
                    line: loc.line,
                    col: loc.column + 1,
                });
            }
            continue;
        }

        let loc = item.span().start();
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
                &src,
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

    if !violations.is_empty() {
        return Err(io::Error::other(violations.join("\n\n")));
    }

    Ok(())
}

pub async fn run_mod_use_adjacency_check(rs_file_path: PathBuf, verbose: bool) -> io::Result<()> {
    if verbose {
        println!(
            "[*] Checking mod/use adjacency {}...",
            rs_file_path.as_path().display()
        );
    }

    let src = read_to_string(&rs_file_path)?;

    let Ok(file) = syn::parse_file(&src) else {
        return Ok(());
    };

    let mut mod_positions: HashMap<String, usize> = HashMap::new();
    for (i, item) in file.items.iter().enumerate() {
        if let Mod(m) = item
            && m.content.is_none()
        {
            mod_positions.insert(m.ident.to_string(), i);
        }
    }

    let mut first_use_for_mod: HashMap<String, usize> = HashMap::new();
    for (i, item) in file.items.iter().enumerate() {
        if let Use(u) = item
            && is_pub_use(u)
            && let Some(seg) = use_tree_first_ident(&u.tree)
            && mod_positions.contains_key(&seg)
        {
            first_use_for_mod.entry(seg).or_insert(i);
        }
    }

    let src_lines: Vec<&str> = src.lines().collect();

    let mut pairs: Vec<(&String, &usize)> = first_use_for_mod.iter().collect();
    pairs.sort_by_key(|(_, use_idx)| **use_idx);

    let mut violations: Vec<String> = Vec::new();

    for (name, use_idx) in pairs {
        let mod_idx = mod_positions[name];
        if *use_idx != mod_idx + 1 {
            let mod_loc = file.items[mod_idx].span().start();
            let mod_col = mod_loc.column + 1;
            let use_loc = file.items[*use_idx].span().start();
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
                &src,
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

    if !violations.is_empty() {
        return Err(io::Error::other(violations.join("\n\n")));
    }

    Ok(())
}

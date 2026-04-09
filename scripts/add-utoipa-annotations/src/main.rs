//! Script to add utoipa::ToSchema annotations to anthropic-api-types.
//!
//! This script processes `rust/src/types.rs` (the merged quicktype output)
//! and adds:
//! 1. Separate `#[derive(utoipa::ToSchema)]` lines for structs and enums.
//! 2. `#[schema(value_type = ...)]` on fields/variants whose type is
//!    `serde_json::Value`, `Option<serde_json::Value>`, `Vec<serde_json::Value>`,
//!    `HashMap<_, serde_json::Value>`, or the quicktype-emitted
//!    `HashMap<String, Option<serde_json::Value>>` (with `Option`/`Vec` wrappers).
//!
//! Features:
//! - Uses syn crate for proper Rust AST parsing
//! - Adds separate derive lines instead of modifying existing ones
//! - Idempotent: skips types/fields that already have the annotations
//! - No import statements added (uses fully qualified paths)
//! - Preserves all comments and original formatting (text insertion, not AST rewrite)
//!
//! Run from the repository root:
//!   cargo run --manifest-path scripts/add-utoipa-annotations/Cargo.toml
//!
//! If a downstream `utoipa::OpenApi` consumer ever stack-overflows on a
//! recursive type, add it to `get_special_case_insertions()`.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Field, Fields, Meta, Type};

const TARGET_FILE: &str = "rust/src/types.rs";

/// Types to skip (contain types that don't implement ToSchema).
/// Currently empty for anthropic-api-types — add entries here if utoipa
/// rejects a specific generated type.
fn get_skip_list() -> HashSet<&'static str> {
    HashSet::new()
}

/// Problematic field types that indicate a type shouldn't get ToSchema.
/// Currently empty — anthropic-api-types only uses primitives, `String`,
/// `Option`, `Vec`, `HashMap`, and `serde_json::Value`. Add entries here
/// if quicktype starts emitting types utoipa cannot handle.
fn get_problematic_types() -> Vec<&'static str> {
    vec![]
}

/// Given a syn::Type, return the appropriate `value_type` string for a
/// `#[schema(value_type = ...)]` annotation, or `None` if not needed.
///
/// Handles:
///   `serde_json::Value`                                       → "Object"
///   `Option<serde_json::Value>`                               → "Option<Object>"
///   `Vec<serde_json::Value>`                                  → "Vec<Object>"
///   `HashMap<_, serde_json::Value>` (any key)                 → "Object"
///   `BTreeMap<_, serde_json::Value>`                          → "Object"
///   `HashMap<_, Option<serde_json::Value>>` (any key)         → "Object"
///   `Option<HashMap<_, Option<serde_json::Value>>>`           → "Option<Object>"
///   `Vec<HashMap<_, Option<serde_json::Value>>>`              → "Vec<Object>"
///   `Option<Vec<HashMap<_, Option<serde_json::Value>>>>`      → "Option<Vec<Object>>"
fn get_schema_value_type(ty: &Type) -> Option<&'static str> {
    let type_str = quote::quote!(#ty).to_string().replace(' ', "");

    if type_str == "serde_json::Value" {
        return Some("Object");
    }
    if type_str == "Option<serde_json::Value>" {
        return Some("Option<Object>");
    }
    if type_str == "Vec<serde_json::Value>" {
        return Some("Vec<Object>");
    }
    if (type_str.starts_with("HashMap<") || type_str.starts_with("BTreeMap<"))
        && type_str.ends_with(",serde_json::Value>")
    {
        return Some("Object");
    }

    // quicktype-emitted polymorphic JSON: HashMap<String, Option<serde_json::Value>>
    // (and Option/Vec wrappers around it)
    let is_open_map = |s: &str| -> bool {
        (s.starts_with("HashMap<") || s.starts_with("BTreeMap<"))
            && s.ends_with(",Option<serde_json::Value>>")
    };

    if is_open_map(&type_str) {
        return Some("Object");
    }
    if let Some(inner) = type_str
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
    {
        if is_open_map(inner) {
            return Some("Option<Object>");
        }
        if let Some(vec_inner) = inner.strip_prefix("Vec<").and_then(|s| s.strip_suffix('>')) {
            if is_open_map(vec_inner) {
                return Some("Option<Vec<Object>>");
            }
        }
    }
    if let Some(inner) = type_str
        .strip_prefix("Vec<")
        .and_then(|s| s.strip_suffix('>'))
    {
        if is_open_map(inner) {
            return Some("Vec<Object>");
        }
    }

    None
}

/// Returns true if `attrs` already contains a `#[schema(value_type = ...)]`.
fn has_schema_value_type_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("schema") {
            return false;
        }
        let tokens = quote::quote!(#attr).to_string().replace(' ', "");
        tokens.contains("value_type=")
    })
}

/// Visitor to check if a type contains problematic field types
struct FieldTypeChecker {
    has_problematic_type: bool,
    problematic_types: Vec<String>,
}

impl FieldTypeChecker {
    fn new() -> Self {
        Self {
            has_problematic_type: false,
            problematic_types: get_problematic_types().iter().map(|s| s.to_string()).collect(),
        }
    }

    fn check_fields(fields: &Fields) -> bool {
        let mut checker = Self::new();
        checker.visit_fields(fields);
        checker.has_problematic_type
    }
}

impl<'ast> Visit<'ast> for FieldTypeChecker {
    fn visit_type(&mut self, ty: &'ast Type) {
        let type_string = quote::quote!(#ty).to_string().replace(' ', "");
        for problematic in &self.problematic_types {
            let p = problematic.replace(' ', "");
            if type_string.contains(&p) {
                self.has_problematic_type = true;
                return;
            }
        }
        syn::visit::visit_type(self, ty);
    }
}

/// Check if derive attributes already contain utoipa::ToSchema
fn has_utoipa_derive(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Meta::List(ref meta_list) = attr.meta {
                let tokens = meta_list.tokens.to_string();
                if tokens.contains("utoipa :: ToSchema") || tokens.contains("utoipa::ToSchema") {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns the end line (1-indexed) of the last `#[derive(...)]` attribute, or 0 if none.
fn get_last_derive_end_line(attrs: &[Attribute]) -> usize {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .map(|attr| attr.span().end().line)
        .max()
        .unwrap_or(0)
}

/// Get the leading whitespace of a specific line (1-indexed) in `content`.
fn get_leading_whitespace(content: &str, line_no: usize) -> String {
    content
        .lines()
        .nth(line_no.saturating_sub(1))
        .map(|line| {
            let trimmed_len = line.len() - line.trim_start().len();
            line[..trimmed_len].to_string()
        })
        .unwrap_or_default()
}

/// A pending annotation insertion into the source text.
#[derive(Debug)]
struct Insertion {
    /// Insert a new line BEFORE this 1-indexed line number.
    before_line: usize,
    /// Leading whitespace for the new line.
    indent: String,
    /// The annotation text (without indentation or trailing newline).
    attr_text: String,
}

/// Apply insertions to the original source, preserving all comments and formatting.
/// Insertions are applied in reverse order so earlier inserts don't shift later line numbers.
fn apply_insertions(content: &str, mut insertions: Vec<Insertion>) -> String {
    // Sort descending by line, then dedup identical (line, text) pairs
    insertions.sort_by(|a, b| {
        b.before_line
            .cmp(&a.before_line)
            .then(a.attr_text.cmp(&b.attr_text))
    });
    insertions.dedup_by(|a, b| a.before_line == b.before_line && a.attr_text == b.attr_text);

    let has_trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    for ins in &insertions {
        let idx = ins.before_line.saturating_sub(1).min(lines.len());
        lines.insert(idx, format!("{}{}", ins.indent, ins.attr_text));
    }

    let mut result = lines.join("\n");
    if has_trailing_newline {
        result.push('\n');
    }
    result
}

/// Visitor that collects needed insertions without modifying the AST.
struct InsertionCollector<'src> {
    content: &'src str,
    insertions: Vec<Insertion>,
    skip_list: HashSet<&'static str>,
}

impl<'src> InsertionCollector<'src> {
    fn new(content: &'src str) -> Self {
        Self {
            content,
            insertions: Vec::new(),
            skip_list: get_skip_list(),
        }
    }

    fn should_add_derive_to_struct(&self, item: &syn::ItemStruct) -> bool {
        let name = item.ident.to_string();
        if self.skip_list.contains(name.as_str()) {
            return false;
        }
        if has_utoipa_derive(&item.attrs) {
            return false;
        }
        if FieldTypeChecker::check_fields(&item.fields) {
            return false;
        }
        true
    }

    fn should_add_derive_to_enum(&self, item: &syn::ItemEnum) -> bool {
        let name = item.ident.to_string();
        if self.skip_list.contains(name.as_str()) {
            return false;
        }
        if has_utoipa_derive(&item.attrs) {
            return false;
        }
        for variant in &item.variants {
            if FieldTypeChecker::check_fields(&variant.fields) {
                return false;
            }
        }
        true
    }

    fn add_derive_insertion(&mut self, attrs: &[Attribute], keyword_line: usize) {
        let last_derive_line = get_last_derive_end_line(attrs);
        let before_line = if last_derive_line > 0 {
            last_derive_line + 1
        } else {
            keyword_line
        };
        self.insertions.push(Insertion {
            before_line,
            indent: String::new(),
            attr_text: "#[derive(utoipa::ToSchema)]".to_string(),
        });
    }

    fn add_field_schema_insertion(&mut self, field: &Field) {
        if has_schema_value_type_attr(&field.attrs) {
            return;
        }
        if let Some(value_type) = get_schema_value_type(&field.ty) {
            // Insert before the line containing the field name (falls back to type span)
            let line_no = field
                .ident
                .as_ref()
                .map(|id| id.span().start().line)
                .unwrap_or_else(|| field.ty.span().start().line);
            let indent = get_leading_whitespace(self.content, line_no);
            self.insertions.push(Insertion {
                before_line: line_no,
                indent,
                attr_text: format!("#[schema(value_type = {value_type})]"),
            });
        }
    }

    fn add_variant_schema_insertion(&mut self, variant: &syn::Variant) {
        if has_schema_value_type_attr(&variant.attrs) {
            return;
        }
        if let syn::Fields::Unnamed(ref unnamed) = variant.fields {
            if unnamed.unnamed.len() == 1 {
                let field = unnamed.unnamed.first().unwrap();
                if let Some(value_type) = get_schema_value_type(&field.ty) {
                    let line_no = variant.ident.span().start().line;
                    let indent = get_leading_whitespace(self.content, line_no);
                    self.insertions.push(Insertion {
                        before_line: line_no,
                        indent,
                        attr_text: format!("#[schema(value_type = {value_type})]"),
                    });
                }
            }
        }
    }
}

impl<'src, 'ast> Visit<'ast> for InsertionCollector<'src> {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if self.should_add_derive_to_struct(item) {
            let kw_line = item.struct_token.span().start().line;
            self.add_derive_insertion(&item.attrs, kw_line);
        }
        if let syn::Fields::Named(ref named) = item.fields {
            for field in &named.named {
                self.add_field_schema_insertion(field);
            }
        }
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if self.should_add_derive_to_enum(item) {
            let kw_line = item.enum_token.span().start().line;
            self.add_derive_insertion(&item.attrs, kw_line);
        }
        for variant in &item.variants {
            // Named fields inside a variant: annotation on the field
            if let syn::Fields::Named(ref named) = variant.fields {
                for field in &named.named {
                    self.add_field_schema_insertion(field);
                }
            }
            // Unnamed tuple variant with single serde_json::Value: annotation on the variant
            self.add_variant_schema_insertion(variant);
        }
    }
}

/// Hardcoded annotations for fields that cannot be auto-detected from their type
/// alone — typically genuine recursive type cycles (e.g. `Vec<SomeParentType>`).
///
/// If downstream `utoipa::OpenApi` registration ever fails with a stack
/// overflow, identify the offending field and add an entry here.
fn get_special_case_insertions(_path: &Path, _content: &str) -> Vec<Insertion> {
    struct SpecialCase {
        /// File must end with this path component (e.g. "types.rs")
        #[allow(dead_code)]
        file_name: &'static str,
        /// Trimmed field declaration to search for
        #[allow(dead_code)]
        field_line: &'static str,
        /// Annotation to insert before the field
        #[allow(dead_code)]
        annotation: &'static str,
    }

    let cases: &[SpecialCase] = &[
        // No known recursive cycles in anthropic-api-types yet.
        // Example entry shape:
        // SpecialCase {
        //     file_name: "types.rs",
        //     field_line: "pub filters: Vec<Filter>,",
        //     annotation: "#[schema(value_type = Vec<Object>)]",
        // },
    ];

    let mut insertions = Vec::new();

    for case in cases {
        if !_path.ends_with(case.file_name) {
            continue;
        }
        for (idx, line) in _content.lines().enumerate() {
            if line.trim() != case.field_line {
                continue;
            }
            // Check if annotation already present among immediately preceding attrs
            let already = _content
                .lines()
                .take(idx)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .take_while(|l| l.trim().starts_with("#["))
                .any(|l| l.contains("value_type"));
            if !already {
                let line_no = idx + 1; // 1-indexed
                insertions.push(Insertion {
                    before_line: line_no,
                    indent: get_leading_whitespace(_content, line_no),
                    attr_text: case.annotation.to_string(),
                });
            }
        }
    }

    insertions
}

/// Process a single Rust file: collect insertions and apply them to the original text.
fn process_file(path: &Path) -> Result<bool> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let syntax_tree: syn::File = match syn::parse_file(&content) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("  ⚠️  Skipping {} (parse error: {})", path.display(), e);
            return Ok(false);
        }
    };

    let mut collector = InsertionCollector::new(&content);
    collector.visit_file(&syntax_tree);

    let mut insertions = collector.insertions;
    insertions.extend(get_special_case_insertions(path, &content));

    if insertions.is_empty() {
        return Ok(false);
    }

    let new_content = apply_insertions(&content, insertions);
    std::fs::write(path, &new_content)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;
    Ok(true)
}

fn main() -> Result<()> {
    println!("🚀 anthropic-api-types utoipa Annotation Script");
    println!("{}", "=".repeat(60));

    let target = Path::new(TARGET_FILE);
    if !target.exists() {
        anyhow::bail!(
            "❌ Error: {} not found!\n\
             Make sure you're running from the anthropic-api-types repository root.\n\
             Example: cargo run --manifest-path scripts/add-utoipa-annotations/Cargo.toml",
            TARGET_FILE
        );
    }

    print!("Processing {}...", TARGET_FILE);
    match process_file(target) {
        Ok(true) => println!(" ✅ Modified"),
        Ok(false) => println!(" ⏭️  No changes needed"),
        Err(e) => {
            println!(" ❌ Failed: {}", e);
            return Err(e);
        }
    }

    println!("\n✅ Annotation script completed successfully!");
    Ok(())
}

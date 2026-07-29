use std::fs;
use std::path::Path;

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::analyzer::{SourceFile, metric_for_bytes};
use crate::model::{FUNCTION_SMALL_SAMPLE_BYTES, FunctionResult};
use crate::{CodensityError, Result};

/// A parser-backed function record retaining its exact source bytes for comparison.
#[derive(Clone, Debug)]
pub(crate) struct ExtractedFunction {
    pub(crate) result: FunctionResult,
    pub(crate) bytes: Vec<u8>,
}

/// Extracts Rust functions, methods, trait methods, and closures from selected source files.
pub(crate) fn extract_rust_functions(files: &[SourceFile]) -> Result<Vec<ExtractedFunction>> {
    let mut functions = Vec::new();
    for file in files
        .iter()
        .filter(|file| crate::LANGUAGES[file.language_index].name == "Rust")
    {
        let source_bytes = fs::read(&file.path).map_err(|source| CodensityError::SourceIo {
            path: file.path.clone(),
            source,
        })?;
        let source = String::from_utf8(source_bytes).map_err(|source| {
            CodensityError::FunctionSourceUtf8 {
                path: file.path.clone(),
                source,
            }
        })?;
        let syntax = syn::parse_file(&source).map_err(|source| CodensityError::FunctionParse {
            path: file.path.clone(),
            source,
        })?;
        let mut collector = FunctionCollector::new(&source, &file.relative);
        collector.visit_file(&syntax);
        functions.extend(collector.finish(&file.path)?);
    }
    functions.sort_by(|left, right| {
        (
            left.result.path.as_bytes(),
            left.result.start_line,
            &left.result.symbol,
        )
            .cmp(&(
                right.result.path.as_bytes(),
                right.result.start_line,
                &right.result.symbol,
            ))
    });
    Ok(functions)
}

struct PendingFunction {
    kind: &'static str,
    symbol: String,
    span: Span,
}

struct FunctionCollector<'source> {
    source: &'source str,
    relative_path: &'source str,
    pending: Vec<PendingFunction>,
    closure_count: u32,
}

impl<'source> FunctionCollector<'source> {
    fn new(source: &'source str, relative_path: &'source str) -> Self {
        Self {
            source,
            relative_path,
            pending: Vec::new(),
            closure_count: 0,
        }
    }

    fn push(&mut self, kind: &'static str, symbol: String, span: Span) {
        self.pending.push(PendingFunction { kind, symbol, span });
    }

    fn finish(self, path: &Path) -> Result<Vec<ExtractedFunction>> {
        let line_starts = line_starts(self.source);
        self.pending
            .into_iter()
            .map(|pending| {
                let (start, end) = span_range(pending.span, &line_starts, self.source.len(), path)?;
                let bytes = self.source.as_bytes()[start..end].to_vec();
                let metric = metric_for_bytes(&bytes, path)?;
                Ok(ExtractedFunction {
                    result: FunctionResult {
                        path: self.relative_path.to_owned(),
                        kind: pending.kind.to_owned(),
                        symbol: pending.symbol,
                        start_line: u32::try_from(pending.span.start().line).map_err(|_| {
                            CodensityError::FunctionSpan {
                                path: path.to_path_buf(),
                            }
                        })?,
                        end_line: u32::try_from(pending.span.end().line).map_err(|_| {
                            CodensityError::FunctionSpan {
                                path: path.to_path_buf(),
                            }
                        })?,
                        small_sample: metric.original_bytes < FUNCTION_SMALL_SAMPLE_BYTES,
                        metric,
                    },
                    bytes,
                })
            })
            .collect()
    }
}

impl<'ast, 'source> Visit<'ast> for FunctionCollector<'source> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.push("function", item.sig.ident.to_string(), item.span());
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.push("method", item.sig.ident.to_string(), item.span());
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.push("trait_method", item.sig.ident.to_string(), item.span());
        syn::visit::visit_trait_item_fn(self, item);
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.closure_count += 1;
        let location = closure.span().start();
        self.push(
            "closure",
            format!(
                "closure-{}-{}-{}",
                self.closure_count, location.line, location.column
            ),
            closure.span(),
        );
        syn::visit::visit_expr_closure(self, closure);
    }
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
    );
    starts
}

fn span_range(span: Span, starts: &[usize], length: usize, path: &Path) -> Result<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    let start_offset = starts
        .get(start.line.saturating_sub(1))
        .and_then(|line| line.checked_add(start.column))
        .filter(|offset| *offset <= length)
        .ok_or_else(|| CodensityError::FunctionSpan {
            path: path.to_path_buf(),
        })?;
    let end_offset = starts
        .get(end.line.saturating_sub(1))
        .and_then(|line| line.checked_add(end.column))
        .filter(|offset| *offset <= length && *offset >= start_offset)
        .ok_or_else(|| CodensityError::FunctionSpan {
            path: path.to_path_buf(),
        })?;
    Ok((start_offset, end_offset))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::extract_rust_functions;
    use crate::analyzer::SourceFile;
    use crate::language::language_for_path;

    #[test]
    fn rust_parser_extracts_functions_methods_trait_methods_and_closures() {
        let path =
            std::env::temp_dir().join(format!("codensity-function-{}.rs", std::process::id()));
        fs::write(
            &path,
            "trait Demo { fn declared(); fn defaulted() {} }\nstruct S;\nimpl S { fn method() { let c = |n: u8| n + 1; let _ = c(1); } }\nfn free() {}\n",
        )
        .expect("write Rust fixture");
        let language_index = language_for_path(&path).expect("recognize Rust");
        let functions = extract_rust_functions(&[SourceFile {
            path: path.clone(),
            relative: "fixture.rs".to_owned(),
            language_index,
        }])
        .expect("extract functions");
        let symbols: Vec<_> = functions
            .iter()
            .map(|function| function.result.symbol.as_str())
            .collect();
        assert!(symbols.contains(&"declared"));
        assert!(symbols.contains(&"defaulted"));
        assert!(symbols.contains(&"method"));
        assert!(symbols.contains(&"free"));
        assert!(symbols.iter().any(|symbol| symbol.starts_with("closure-")));
        assert!(
            functions
                .iter()
                .all(|function| function.result.path == "fixture.rs")
        );
        fs::remove_file(path).expect("remove Rust fixture");
    }
}

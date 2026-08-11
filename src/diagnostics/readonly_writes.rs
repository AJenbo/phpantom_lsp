//! Readonly property write diagnostics.
//!
//! A `readonly` property may be initialized exactly once, and only from
//! inside the class that declares it.  Every other write is a fatal
//! `Error` at runtime:
//!
//! ```php
//! final class Box {
//!     public function __construct(public readonly int $value) {}
//! }
//!
//! $box = new Box(1);
//! $box->value = 2; // Cannot modify readonly property Box::$value
//! ```
//!
//! Two shapes are reported:
//!
//! - **Outside the declaring class** — the write happens in a scope
//!   that is not the declaring class (top-level code, another class, a
//!   subclass initializing its parent's property).  This is an error
//!   regardless of whether the property was ever initialized.
//! - **After the constructor initialized it** — the write is inside the
//!   declaring class, but the constructor is known to have initialized
//!   the property already (it is a promoted parameter, or the
//!   constructor body assigns it unconditionally), so the write is a
//!   second initialization.
//!
//! Writes inside `__construct`, `__clone` (PHP 8.3 allows readonly
//! reinitialization while cloning), and `__unserialize` are never
//! flagged, and neither is a write whose target the constructor may
//! leave uninitialized: initializing a readonly property lazily from
//! another method of the declaring class is legal PHP.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::access::Access;
use mago_syntax::cst::class_like::member::{ClassLikeMember, ClassLikeMemberSelector};
use mago_syntax::cst::class_like::method::MethodBody;
use mago_syntax::cst::class_like::{AnonymousClass, Class};
use mago_syntax::cst::expression::Expression;
use mago_syntax::cst::sequence::Sequence;
use mago_syntax::cst::statement::Statement;
use mago_syntax::cst::unary::UnaryPrefixOperator;
use mago_syntax::cst::variable::Variable;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::atom::{Atom, bytes_to_str};
use crate::hover::{MemberKindForOrigin, find_declaring_class};
use crate::parser::{with_parse_cache, with_parsed_program};
use crate::symbol_map::SymbolKind;
use crate::type_engine::resolver::{ResolutionCtx, SubjectOutcome, resolve_subject_outcome};
use crate::types::{AccessKind, ClassInfo, ClassLikeKind};
use crate::virtual_members::resolve_class_fully_cached;

use super::helpers::{FileDiagnosticContext, find_innermost_enclosing_class, make_diagnostic};

/// Diagnostic code used for writes to a `readonly` property.
pub(crate) const INVALID_READONLY_WRITE_CODE: &str = "invalid_readonly_write";

// ── Collected write sites ───────────────────────────────────────────────────

/// Everything the file's AST contributes to the check.
#[derive(Default)]
struct FileWrites {
    /// Byte offsets of property identifiers that appear as the target of
    /// a write.  This is the same offset the symbol map records as the
    /// start of the matching `MemberAccess` span.
    targets: HashSet<u32>,
    /// Per class body, keyed by the left-brace offset so it matches
    /// [`ClassInfo::start_offset`].
    classes: HashMap<u32, ClassInitFacts>,
}

/// What a class body says about its own readonly initialization.
#[derive(Default)]
struct ClassInitFacts {
    /// Body ranges of `__construct`, `__clone`, and `__unserialize`,
    /// where a readonly property may legitimately be initialized.
    initializer_ranges: Vec<(u32, u32)>,
    /// Properties the constructor is guaranteed to have initialized by
    /// the time any other method runs: promoted parameters and
    /// unconditional `$this->name = …` statements in its body.
    initialized: HashSet<String>,
}

impl ClassInitFacts {
    fn covers_initializer(&self, offset: u32) -> bool {
        self.initializer_ranges
            .iter()
            .any(|(start, end)| offset >= *start && offset <= *end)
    }
}

// ── AST walk ────────────────────────────────────────────────────────────────

struct WriteWalker;

impl<'ast, 'arena> mago_syntax::walker::Walker<'ast, 'arena, FileWrites> for WriteWalker {
    fn walk_in_expression(&self, node: &'ast Expression<'arena>, ctx: &mut FileWrites) {
        match node {
            Expression::Assignment(assign) => record_target(assign.lhs, ctx),
            Expression::UnaryPrefix(unary)
                if matches!(
                    unary.operator,
                    UnaryPrefixOperator::PreIncrement(_) | UnaryPrefixOperator::PreDecrement(_)
                ) =>
            {
                record_target(unary.operand, ctx)
            }
            // The only postfix operators are `++` and `--`.
            Expression::UnaryPostfix(unary) => record_target(unary.operand, ctx),
            _ => {}
        }
    }

    fn walk_in_class(&self, node: &'ast Class<'arena>, ctx: &mut FileWrites) {
        ctx.classes.insert(
            node.left_brace.start.offset,
            class_init_facts(&node.members),
        );
    }

    fn walk_in_anonymous_class(&self, node: &'ast AnonymousClass<'arena>, ctx: &mut FileWrites) {
        ctx.classes.insert(
            node.left_brace.start.offset,
            class_init_facts(&node.members),
        );
    }
}

/// Record `expr` as a write target when it is a named property access.
///
/// Dynamic selectors (`$obj->$name`) and null-safe access (never a valid
/// write target) are skipped, as are static properties: PHP rejects
/// `static readonly` outright.
fn record_target(expr: &Expression<'_>, ctx: &mut FileWrites) {
    if let Expression::Access(Access::Property(access)) = expr
        && let ClassLikeMemberSelector::Identifier(ident) = &access.property
    {
        ctx.targets.insert(ident.span.start.offset);
    }
}

fn class_init_facts(members: &Sequence<'_, ClassLikeMember<'_>>) -> ClassInitFacts {
    let mut facts = ClassInitFacts::default();

    for member in members.iter() {
        let ClassLikeMember::Method(method) = member else {
            continue;
        };
        let name = bytes_to_str(method.name.value);
        let is_constructor = name.eq_ignore_ascii_case("__construct");
        if !is_constructor
            && !name.eq_ignore_ascii_case("__clone")
            && !name.eq_ignore_ascii_case("__unserialize")
        {
            continue;
        }
        let MethodBody::Concrete(block) = &method.body else {
            continue;
        };
        let span = block.span();
        facts
            .initializer_ranges
            .push((span.start.offset, span.end.offset));

        if !is_constructor {
            continue;
        }

        for param in method.parameter_list.parameters.iter() {
            if param.is_promoted_property() {
                let raw = bytes_to_str(param.variable.name);
                facts
                    .initialized
                    .insert(raw.strip_prefix('$').unwrap_or(raw).to_string());
            }
        }

        // Only statements at the top level of the constructor body count:
        // an assignment inside a branch or a loop may not run, which
        // leaves the property free to be initialized elsewhere.
        for statement in block.statements.iter() {
            if let Statement::Expression(stmt) = statement
                && let Expression::Assignment(assign) = stmt.expression
                && assign.operator.is_assign()
                && let Expression::Access(Access::Property(access)) = assign.lhs
                && is_this(access.object)
                && let ClassLikeMemberSelector::Identifier(ident) = &access.property
            {
                facts
                    .initialized
                    .insert(bytes_to_str(ident.value).to_string());
            }
        }
    }

    facts
}

fn is_this(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Variable(Variable::Direct(var)) if var.name == b"$this")
}

// ── Verdicts ────────────────────────────────────────────────────────────────

enum Verdict {
    /// Nothing to report for this receiver type.
    Ok,
    /// The write is not in the property's declaring scope.
    OutsideDeclaringClass(String),
    /// The write is in the declaring scope, but the constructor has
    /// already initialized the property.
    AlreadyInitialized(String),
}

/// Whether `class` declares `property` itself, following its traits.
///
/// Trait properties are flattened into the using class, so a class that
/// uses a trait declaring a readonly property *is* the declaring scope
/// for that property as far as PHP is concerned.
fn declares_property(
    class: &ClassInfo,
    property: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    seen: &mut Vec<Atom>,
) -> bool {
    if class
        .properties
        .iter()
        .any(|p| p.name == property && !p.is_virtual)
    {
        return true;
    }

    for trait_name in &class.used_traits {
        if seen.contains(trait_name) {
            continue;
        }
        seen.push(*trait_name);
        if let Some(used) = class_loader(trait_name)
            && declares_property(&used, property, class_loader, seen)
        {
            return true;
        }
    }

    false
}

/// How the property is spelled in the diagnostic message.
fn owner_display(declaring: &ClassInfo, property: &str) -> String {
    if declaring.name.starts_with("__anonymous@") {
        format!("${}", property)
    } else {
        format!("{}::${}", declaring.fqn(), property)
    }
}

// ── Main diagnostic collection ──────────────────────────────────────────────

impl Backend {
    /// Collect readonly-write diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub(crate) fn collect_readonly_write_diagnostics_with_context(
        &self,
        ctx: &FileDiagnosticContext,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        let _parse_guard = with_parse_cache(content);

        let writes = with_parsed_program(content, "readonly_writes", |program, _content| {
            let mut writes = FileWrites::default();
            let walker = WriteWalker;
            for statement in program.statements.iter() {
                mago_syntax::walker::Walker::walk_statement(&walker, statement, &mut writes);
            }
            writes
        });

        if writes.targets.is_empty() {
            return;
        }

        let local_classes = &ctx.file.classes;
        let class_loader =
            self.class_loader_with(local_classes, &ctx.file.use_map, &ctx.file.namespace);
        let function_loader = self.function_loader_with(
            ctx.file.resolved_names.as_deref(),
            &ctx.file.use_map,
            &ctx.file.namespace,
        );
        let resolved_cache = &self.resolved_class_cache;
        let symbol_map = &ctx.symbol_map;

        for span in &symbol_map.spans {
            let SymbolKind::MemberAccess {
                subject_text,
                member_name,
                is_static,
                is_method_call,
                ..
            } = &span.kind
            else {
                continue;
            };
            if *is_static || *is_method_call || !writes.targets.contains(&span.start) {
                continue;
            }

            let current_class = find_innermost_enclosing_class(local_classes, span.start);

            // A trait body is incomplete by nature: the property it
            // writes may be declared by any host class, and the trait
            // itself is flattened into that host's scope.
            if current_class.is_some_and(|class| class.kind == ClassLikeKind::Trait) {
                continue;
            }

            let subject_text = subject_text.as_str(content);

            // `$this->ownProperty = …` inside the constructor is the
            // shape most writes in a file have.  Settling it before
            // resolving the receiver keeps the common case off the type
            // engine.  A property the enclosing class does not declare
            // itself still goes through the full check: initializing a
            // parent's readonly property is an error even in a
            // constructor.
            if subject_text == "$this"
                && current_class.is_some_and(|class| {
                    writes
                        .classes
                        .get(&class.start_offset)
                        .is_some_and(|facts| facts.covers_initializer(span.start))
                        && declares_property(class, member_name, &class_loader, &mut Vec::new())
                })
            {
                continue;
            }

            let rctx = ResolutionCtx {
                current_class,
                all_classes: local_classes,
                content,
                cursor_offset: span.start,
                class_loader: &class_loader,
                backend: Some(self),
                laravel_macro_this_resolver: None,
                resolved_class_cache: Some(resolved_cache),
                function_loader: Some(&function_loader),
                scope_var_resolver: None,
                is_in_static_method: symbol_map.is_in_static_method(span.start),
                preserve_static: false,
            };
            let SubjectOutcome::Resolved(receivers) =
                resolve_subject_outcome(subject_text, AccessKind::Arrow, &rctx)
            else {
                continue;
            };

            // Every branch of a union has to be invalid before the write
            // is: a value that could hold the branch where the property
            // is writable makes the assignment legal.
            let mut verdict: Option<Verdict> = None;
            for receiver in &receivers {
                let judged = self.judge_readonly_write(
                    receiver,
                    member_name,
                    current_class,
                    span.start,
                    &writes,
                    &class_loader,
                );
                if matches!(judged, Verdict::Ok) {
                    verdict = None;
                    break;
                }
                verdict.get_or_insert(judged);
            }

            let message = match verdict {
                Some(Verdict::OutsideDeclaringClass(owner)) => format!(
                    "Cannot modify readonly property {} from outside its declaring class",
                    owner
                ),
                Some(Verdict::AlreadyInitialized(owner)) => format!(
                    "Cannot modify readonly property {} after the constructor has initialized it",
                    owner
                ),
                _ => continue,
            };

            let Some(range) = self.offset_range_to_lsp_range(
                uri,
                content,
                span.start as usize,
                span.end as usize,
            ) else {
                continue;
            };

            out.push(make_diagnostic(
                range,
                DiagnosticSeverity::ERROR,
                INVALID_READONLY_WRITE_CODE,
                message,
            ));
        }
    }

    /// Judge a single write against one of the receiver's possible types.
    fn judge_readonly_write(
        &self,
        receiver: &Arc<ClassInfo>,
        property: &str,
        current_class: Option<&ClassInfo>,
        offset: u32,
        writes: &FileWrites,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> Verdict {
        // Object shapes carry all their members already and must not go
        // through the cache: every shape shares the same class name.
        let merged = if receiver.name == "__object_shape" {
            Arc::clone(receiver)
        } else {
            resolve_class_fully_cached(receiver, class_loader, &self.resolved_class_cache)
        };

        let is_readonly = merged
            .properties
            .iter()
            .any(|p| p.name == property && p.is_readonly && !p.is_virtual);
        if !is_readonly {
            return Verdict::Ok;
        }

        // The declaring class has to be traced through the *unmerged*
        // class: a merged one lists inherited properties as its own.
        let raw = class_loader(merged.fqn().as_str()).unwrap_or_else(|| Arc::clone(receiver));
        let declaring =
            find_declaring_class(&raw, property, &MemberKindForOrigin::Property, class_loader);
        let owner = owner_display(&declaring, property);

        let Some(current) = current_class else {
            return Verdict::OutsideDeclaringClass(owner);
        };

        let in_declaring_scope = current.fqn() == declaring.fqn()
            || (declaring.kind == ClassLikeKind::Trait
                && declares_property(current, property, class_loader, &mut Vec::new()));
        if !in_declaring_scope {
            return Verdict::OutsideDeclaringClass(owner);
        }

        let Some(facts) = writes.classes.get(&current.start_offset) else {
            return Verdict::Ok;
        };
        if facts.covers_initializer(offset) || !facts.initialized.contains(property) {
            return Verdict::Ok;
        }

        Verdict::AlreadyInitialized(owner)
    }
}

//! Argument count diagnostics.
//!
//! Walk the precomputed [`CallSite`] entries in the symbol map and flag
//! every call that passes too few or too many arguments relative to the
//! resolved callable's parameter list.
//!
//! Diagnostics use `Severity::Error` because passing the wrong number
//! of arguments crashes at runtime with a `TypeError`.
//!
//! Suppression rules:
//! - Calls that cannot be resolved are skipped (we already have
//!   unknown-function and unknown-member diagnostics for those).
//! - Calls that use argument unpacking (`...$args`) are skipped because
//!   the actual argument count is unknown at static analysis time.
//! - Methods routed through `__call` / `__callStatic` are skipped
//!   because the magic method accepts arbitrary arguments.
//! - Constructor calls on classes with no explicit `__construct` are
//!   skipped (PHP allows `new Foo()` even without a constructor).

use tower_lsp::lsp_types::*;

use crate::Backend;

use super::helpers::make_diagnostic;
use super::offset_range_to_lsp_range;

/// Diagnostic code used for argument-count diagnostics.
pub(crate) const ARGUMENT_COUNT_CODE: &str = "argument_count";

impl Backend {
    /// Collect argument-count diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub fn collect_argument_count_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Gather context under locks ──────────────────────────────
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_ctx = self.file_context(uri);

        // ── Walk every call site ────────────────────────────────────
        for call_site in &symbol_map.call_sites {
            // Skip calls with argument unpacking — actual count is
            // unknown at static analysis time.
            if call_site.has_unpacking {
                continue;
            }

            let expr = &call_site.call_expression;

            // Build an LSP Position from the call site's args_start
            // offset.  `resolve_callable_target` needs a position for
            // class context resolution (`$this`, `self`, `static`).
            let position = crate::util::offset_to_position(content, call_site.args_start as usize);

            // Resolve the call expression to a callable target.
            let resolved = match self.resolve_callable_target(expr, content, position, &file_ctx) {
                Some(r) => r,
                None => continue,
            };

            let params = &resolved.parameters;
            let actual_args = call_site.arg_count;

            // Count required parameters (no default, not variadic).
            let required_count = params.iter().filter(|p| p.is_required).count() as u32;

            // Count total non-variadic parameters.
            let has_variadic = params.iter().any(|p| p.is_variadic);
            let max_count = if has_variadic {
                None // unlimited trailing args
            } else {
                Some(params.len() as u32)
            };

            // ── Too few arguments ───────────────────────────────────
            if actual_args < required_count {
                let range = match offset_range_to_lsp_range(
                    content,
                    call_site.args_start.saturating_sub(1) as usize,
                    call_site.args_end.saturating_add(1) as usize,
                ) {
                    Some(r) => r,
                    None => continue,
                };

                let message = if has_variadic {
                    format!(
                        "Expected at least {} argument{}, got {}",
                        required_count,
                        if required_count == 1 { "" } else { "s" },
                        actual_args,
                    )
                } else if required_count == max_count.unwrap_or(0) {
                    format!(
                        "Expected {} argument{}, got {}",
                        required_count,
                        if required_count == 1 { "" } else { "s" },
                        actual_args,
                    )
                } else {
                    format!(
                        "Expected at least {} argument{}, got {}",
                        required_count,
                        if required_count == 1 { "" } else { "s" },
                        actual_args,
                    )
                };

                out.push(make_diagnostic(
                    range,
                    DiagnosticSeverity::ERROR,
                    ARGUMENT_COUNT_CODE,
                    message,
                ));
                continue;
            }

            // ── Too many arguments ──────────────────────────────────
            if !self.config().diagnostics.extra_arguments_enabled() {
                continue;
            }

            if let Some(max) = max_count
                && actual_args > max
            {
                let range = match offset_range_to_lsp_range(
                    content,
                    call_site.args_start.saturating_sub(1) as usize,
                    call_site.args_end.saturating_add(1) as usize,
                ) {
                    Some(r) => r,
                    None => continue,
                };

                let message = if required_count == max {
                    format!(
                        "Expected {} argument{}, got {}",
                        max,
                        if max == 1 { "" } else { "s" },
                        actual_args,
                    )
                } else {
                    format!(
                        "Expected at most {} argument{}, got {}",
                        max,
                        if max == 1 { "" } else { "s" },
                        actual_args,
                    )
                };

                out.push(make_diagnostic(
                    range,
                    DiagnosticSeverity::ERROR,
                    ARGUMENT_COUNT_CODE,
                    message,
                ));
            }
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Enable the `extra-arguments` diagnostic on the given backend.
    fn enable_extra_args(backend: &Backend) {
        let mut cfg = backend.config.lock().clone();
        cfg.diagnostics.extra_arguments = Some(true);
        *backend.config.lock() = cfg;
    }

    /// Helper: create a test backend with minimal function stubs and
    /// collect argument-count diagnostics.  Extra-arguments checking
    /// is **off** (the default).
    fn collect(php: &str) -> Vec<Diagnostic> {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        backend.update_ast(uri, php);
        let mut out = Vec::new();
        backend.collect_argument_count_diagnostics(uri, php, &mut out);
        out
    }

    /// Like [`collect`] but with the `extra-arguments` diagnostic
    /// enabled so that "too many arguments" errors are reported.
    fn collect_extra(php: &str) -> Vec<Diagnostic> {
        let backend = Backend::new_test();
        enable_extra_args(&backend);
        let uri = "file:///test.php";
        backend.update_ast(uri, php);
        let mut out = Vec::new();
        backend.collect_argument_count_diagnostics(uri, php, &mut out);
        out
    }

    /// Minimal stub function index shared by stub-aware helpers.
    fn stub_fn_index() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("strlen", "<?php\nfunction strlen(string $string): int {}\n"),
            (
                "array_map",
                "<?php\nfunction array_map(?callable $callback, array $array, array ...$arrays): array {}\n",
            ),
            (
                "implode",
                "<?php\nfunction implode(string $separator, array $array): string {}\n",
            ),
            (
                "str_replace",
                "<?php\nfunction str_replace(string|array $search, string|array $replace, string|array $subject): string|array {}\n",
            ),
            (
                "array_push",
                "<?php\nfunction array_push(array &$array, mixed ...$values): int {}\n",
            ),
            (
                "in_array",
                "<?php\nfunction in_array(mixed $needle, array $haystack, bool $strict = false): bool {}\n",
            ),
            (
                "substr",
                "<?php\nfunction substr(string $string, int $offset, ?int $length = null): string {}\n",
            ),
        ])
    }

    /// Helper that includes minimal stub functions so that built-in
    /// functions like `strlen` are resolvable.  Extra-arguments
    /// checking is **off** (the default).
    fn collect_with_stubs(php: &str) -> Vec<Diagnostic> {
        let backend =
            Backend::new_test_with_all_stubs(HashMap::new(), stub_fn_index(), HashMap::new());
        let uri = "file:///test.php";
        backend.update_ast(uri, php);
        let mut out = Vec::new();
        backend.collect_argument_count_diagnostics(uri, php, &mut out);
        out
    }

    /// Like [`collect_with_stubs`] but with the `extra-arguments`
    /// diagnostic enabled.
    fn collect_with_stubs_extra(php: &str) -> Vec<Diagnostic> {
        let backend =
            Backend::new_test_with_all_stubs(HashMap::new(), stub_fn_index(), HashMap::new());
        enable_extra_args(&backend);
        let uri = "file:///test.php";
        backend.update_ast(uri, php);
        let mut out = Vec::new();
        backend.collect_argument_count_diagnostics(uri, php, &mut out);
        out
    }

    // ── Too few arguments ───────────────────────────────────────────

    #[test]
    fn flags_too_few_args_to_function() {
        let php = r#"<?php
function test(): void {
    strlen();
}
"#;
        let diags = collect_with_stubs(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert!(
            diags[0].message.contains("Expected 1 argument"),
            "message: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("got 0"),
            "message: {}",
            diags[0].message,
        );
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn flags_too_few_args_to_method() {
        let php = r#"<?php
class Greeter {
    public function greet(string $name): string {
        return "Hello, " . $name;
    }
}
function test(): void {
    $g = new Greeter();
    $g->greet();
}
"#;
        let diags = collect(php);
        assert!(
            diags.iter().any(|d| d.message.contains("got 0")),
            "Expected too-few-args diagnostic, got: {diags:?}",
        );
    }

    #[test]
    fn flags_too_few_args_to_static_method() {
        let php = r#"<?php
class Math {
    public static function add(int $a, int $b): int {
        return $a + $b;
    }
}
function test(): void {
    Math::add(1);
}
"#;
        let diags = collect(php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Expected 2 arguments") && d.message.contains("got 1")),
            "Expected too-few-args diagnostic, got: {diags:?}",
        );
    }

    // ── Too many arguments (default off) ────────────────────────────

    #[test]
    fn too_many_args_suppressed_by_default() {
        let php = r#"<?php
function test(): void {
    strlen("hello", "extra");
}
"#;
        let diags = collect_with_stubs(php);
        assert!(
            diags.is_empty(),
            "Extra-arguments diagnostic should be off by default, got: {diags:?}",
        );
    }

    #[test]
    fn too_many_args_to_user_function_suppressed_by_default() {
        let php = r#"<?php
function myHelper(string $a): void {}
function test(): void {
    myHelper("x", "y");
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Extra-arguments diagnostic should be off by default, got: {diags:?}",
        );
    }

    #[test]
    fn too_many_args_to_method_suppressed_by_default() {
        let php = r#"<?php
class Greeter {
    public function greet(string $name): string {
        return "Hello, " . $name;
    }
}
function test(): void {
    $g = new Greeter();
    $g->greet("world", "extra", "more");
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Extra-arguments diagnostic should be off by default, got: {diags:?}",
        );
    }

    // ── Too many arguments (opt-in) ─────────────────────────────────

    #[test]
    fn flags_too_many_args_to_function() {
        let php = r#"<?php
function test(): void {
    strlen("hello", "extra");
}
"#;
        let diags = collect_with_stubs_extra(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert!(
            diags[0].message.contains("got 2"),
            "message: {}",
            diags[0].message,
        );
    }

    #[test]
    fn flags_too_many_args_to_method() {
        let php = r#"<?php
class Greeter {
    public function greet(string $name): string {
        return "Hello, " . $name;
    }
}
function test(): void {
    $g = new Greeter();
    $g->greet("world", "extra", "more");
}
"#;
        let diags = collect_extra(php);
        assert!(
            diags.iter().any(|d| d.message.contains("got 3")),
            "Expected too-many-args diagnostic, got: {diags:?}",
        );
    }

    // ── Correct argument count — no diagnostic ──────────────────────

    #[test]
    fn no_diagnostic_for_correct_arg_count() {
        let php = r#"<?php
function test(): void {
    strlen("hello");
}
"#;
        let diags = collect_with_stubs(php);
        assert!(diags.is_empty(), "No diagnostics expected, got: {diags:?}",);
    }

    #[test]
    fn no_diagnostic_with_optional_args() {
        let php = r#"<?php
function test(): void {
    in_array("x", ["x", "y"]);
    in_array("x", ["x", "y"], true);
}
"#;
        let diags = collect_with_stubs(php);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for optional args, got: {diags:?}",
        );
    }

    #[test]
    fn no_diagnostic_with_default_value() {
        let php = r#"<?php
function test(): void {
    substr("hello", 1);
    substr("hello", 1, 3);
}
"#;
        let diags = collect_with_stubs(php);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for default-valued params, got: {diags:?}",
        );
    }

    // ── Variadic functions ──────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_extra_args_to_variadic_function() {
        let php = r#"<?php
function test(): void {
    array_map(null, [1], [2], [3], [4]);
}
"#;
        let diags = collect_with_stubs(php);
        assert!(
            diags.is_empty(),
            "Variadic function should accept extra args, got: {diags:?}",
        );
    }

    #[test]
    fn flags_too_few_required_args_to_variadic_function() {
        let php = r#"<?php
function test(): void {
    array_push();
}
"#;
        let diags = collect_with_stubs(php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("at least 1 argument")),
            "Expected too-few-args diagnostic for variadic function, got: {diags:?}",
        );
    }

    // ── Argument unpacking suppression ──────────────────────────────

    #[test]
    fn no_diagnostic_when_args_are_unpacked() {
        let php = r#"<?php
function test(): void {
    $args = ["hello"];
    strlen(...$args);
}
"#;
        let diags = collect_with_stubs(php);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when using argument unpacking, got: {diags:?}",
        );
    }

    // ── Unresolvable calls ──────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_unresolvable_function() {
        let php = r#"<?php
function test(): void {
    nonExistentFunction(1, 2, 3);
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "No arg-count diagnostics expected for unresolvable functions, got: {diags:?}",
        );
    }

    // ── Same-file user-defined functions ─────────────────────────────

    #[test]
    fn flags_too_few_args_to_user_function() {
        let php = r#"<?php
function myHelper(string $a, int $b): void {}
function test(): void {
    myHelper("x");
}
"#;
        let diags = collect(php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Expected 2") && d.message.contains("got 1")),
            "Expected too-few-args diagnostic, got: {diags:?}",
        );
    }

    #[test]
    fn flags_too_many_args_to_user_function() {
        let php = r#"<?php
function myHelper(string $a): void {}
function test(): void {
    myHelper("x", "y");
}
"#;
        let diags = collect_extra(php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Expected 1 argument") && d.message.contains("got 2")),
            "Expected too-many-args diagnostic, got: {diags:?}",
        );
    }

    #[test]
    fn no_diagnostic_for_correct_user_function_call() {
        let php = r#"<?php
function myHelper(string $a, int $b = 0): void {}
function test(): void {
    myHelper("x");
    myHelper("x", 1);
}
"#;
        let diags = collect(php);
        assert!(diags.is_empty(), "No diagnostics expected, got: {diags:?}",);
    }

    // ── Diagnostic metadata ─────────────────────────────────────────

    #[test]
    fn diagnostic_has_correct_code_and_source() {
        let php = r#"<?php
function myHelper(string $a): void {}
function test(): void {
    myHelper();
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("argument_count".to_string())),
        );
        assert_eq!(diags[0].source, Some("phpantom".to_string()));
    }

    // ── Constructor calls ───────────────────────────────────────────

    #[test]
    fn flags_too_few_args_to_constructor() {
        let php = r#"<?php
class User {
    public function __construct(string $name, string $email) {}
}
function test(): void {
    new User("Alice");
}
"#;
        let diags = collect(php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Expected 2") && d.message.contains("got 1")),
            "Expected too-few-args diagnostic for constructor, got: {diags:?}",
        );
    }

    #[test]
    fn flags_too_many_args_to_constructor() {
        let php = r#"<?php
class User {
    public function __construct(string $name) {}
}
function test(): void {
    new User("Alice", "extra");
}
"#;
        let diags = collect_extra(php);
        assert!(
            diags.iter().any(|d| d.message.contains("got 2")),
            "Expected too-many-args diagnostic for constructor, got: {diags:?}",
        );
    }

    #[test]
    fn no_diagnostic_for_correct_constructor() {
        let php = r#"<?php
class User {
    public function __construct(string $name, string $email = "") {}
}
function test(): void {
    new User("Alice");
    new User("Alice", "alice@test.com");
}
"#;
        let diags = collect(php);
        assert!(diags.is_empty(), "No diagnostics expected, got: {diags:?}",);
    }

    // ── "at least / at most" message wording ────────────────────────

    #[test]
    fn message_says_at_least_when_some_params_optional() {
        let php = r#"<?php
function helper(string $a, string $b, string $c = ""): void {}
function test(): void {
    helper("x");
}
"#;
        let diags = collect(php);
        assert!(
            diags.iter().any(|d| d.message.contains("at least 2")),
            "Expected 'at least' wording, got: {diags:?}",
        );
    }

    #[test]
    fn message_says_at_most_when_too_many_with_optional() {
        let php = r#"<?php
function helper(string $a, string $b = ""): void {}
function test(): void {
    helper("x", "y", "z");
}
"#;
        let diags = collect_extra(php);
        assert!(
            diags.iter().any(|d| d.message.contains("at most 2")),
            "Expected 'at most' wording, got: {diags:?}",
        );
    }

    // ── Multiple diagnostics ────────────────────────────────────────

    #[test]
    fn flags_multiple_bad_calls() {
        let php = r#"<?php
function one(int $a): void {}
function two(int $a, int $b): void {}
function test(): void {
    one();
    two(1, 2, 3);
}
"#;
        let diags = collect_extra(php);
        assert_eq!(diags.len(), 2, "Expected 2 diagnostics, got: {diags:?}",);
    }

    #[test]
    fn too_few_still_reported_when_extra_args_disabled() {
        // "Too few" must always fire regardless of the extra-arguments flag.
        let php = r#"<?php
function one(int $a): void {}
function two(int $a, int $b): void {}
function test(): void {
    one();
    two(1, 2, 3);
}
"#;
        let diags = collect(php);
        assert_eq!(
            diags.len(),
            1,
            "Only the too-few diagnostic should fire by default, got: {diags:?}",
        );
        assert!(
            diags[0].message.contains("got 0"),
            "message: {}",
            diags[0].message,
        );
    }

    // ── Scope methods (Laravel) ─────────────────────────────────────

    #[test]
    fn no_diagnostic_for_scope_method_with_query_stripped() {
        // #[Scope]-attributed methods have their first $query parameter
        // stripped by the virtual member provider.  The arg count
        // diagnostic must see the virtual method (0 required params),
        // not the original (1 required param).
        let php = r#"<?php
namespace Illuminate\Database\Eloquent\Attributes;

#[\Attribute]
class Scope {}

namespace Illuminate\Database\Eloquent;

class Model {}
class Builder {}

namespace App;

use Illuminate\Database\Eloquent\Model;

class Bakery extends Model {
    #[\Illuminate\Database\Eloquent\Attributes\Scope]
    protected function fresh(\Illuminate\Database\Eloquent\Builder $query): void {
        $query->where('fresh', true);
    }
}

class Demo {
    public function test(): void {
        $bakery = new Bakery();
        $bakery->fresh();
    }
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Scope method with $query stripped should accept 0 args, got: {diags:?}",
        );
    }

    #[test]
    fn flags_too_few_args_to_scope_method_with_extra_param() {
        // scopeTopping($query, $type) → virtual topping($type) needs 1 arg.
        let php = r#"<?php
namespace Illuminate\Database\Eloquent\Attributes;

#[\Attribute]
class Scope {}

namespace Illuminate\Database\Eloquent;

class Model {}
class Builder {}

namespace App;

use Illuminate\Database\Eloquent\Model;

class Bakery extends Model {
    public function scopeTopping(\Illuminate\Database\Eloquent\Builder $query, string $type): void {
        $query->where('topping', $type);
    }
}

class Demo {
    public function test(): void {
        $bakery = new Bakery();
        $bakery->topping();
    }
}
"#;
        let diags = collect(php);
        assert!(
            diags.iter().any(|d| d.message.contains("got 0")),
            "Scope method topping() needs 1 arg after $query stripping, got: {diags:?}",
        );
    }
}

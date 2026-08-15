//! The proofs a body carries from where they are written to where they
//! are read.
//!
//! Each case here is a guard or an assignment the source really makes and
//! the walker used to drop on the way to the read: a static property
//! written and then returned, an assignment inside a `try` whose `catch`
//! rethrows, an `&&` chain in a match arm, an identity check against an
//! enum case, and a loop condition that narrows its own operands. Losing
//! any of them surfaces as a `?T` where the source proved `T`, so the
//! null-argument and return-type diagnostics are what these assert on.

use crate::common::create_test_backend;
use tower_lsp::lsp_types::*;

/// Run the slow diagnostic pipeline (which activates the forward
/// walker's scope cache, as a real analysis does) and keep only the
/// diagnostics a lost narrowing produces.
fn type_diagnostics(php: &str) -> Vec<String> {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    backend.update_ast(uri, php);
    let mut out = Vec::new();
    backend.collect_slow_diagnostics(uri, php, &mut out);
    out.iter()
        .filter(|d| {
            d.code.as_ref().is_some_and(|c| {
                matches!(c, NumberOrString::String(s)
                    if s == "type_mismatch_argument" || s == "type_mismatch_return")
            })
        })
        .map(|d| d.message.clone())
        .collect()
}

fn assert_no_type_errors(php: &str) {
    let messages = type_diagnostics(php);
    assert!(
        messages.is_empty(),
        "expected the narrowing to reach the read, got: {messages:?}"
    );
}

fn assert_type_error(php: &str) {
    let messages = type_diagnostics(php);
    assert_eq!(
        messages.len(),
        1,
        "expected exactly one type error, got: {messages:?}"
    );
}

// ─── Static properties ──────────────────────────────────────────────────────

const STATIC_SCAFFOLD: &str = r#"<?php
namespace Repro;

class Repo {}

class Registry {
    private static ?Repo $repo = null;

    public static function takes(Repo $r): void {}
"#;

#[test]
fn the_lazy_init_idiom_leaves_no_null_behind() {
    assert_no_type_errors(&format!(
        r#"{STATIC_SCAFFOLD}
    public static function repo(): Repo
    {{
        if (self::$repo === null) {{
            self::$repo = new Repo();
        }}

        return self::$repo;
    }}
}}
"#
    ));
}

#[test]
fn a_write_to_a_static_property_is_what_the_next_read_sees() {
    assert_no_type_errors(&format!(
        r#"{STATIC_SCAFFOLD}
    public static function go(): void
    {{
        self::$repo = new Repo();
        self::takes(self::$repo);
    }}
}}
"#
    ));
}

#[test]
fn a_guard_that_throws_proves_the_static_property_afterwards() {
    assert_no_type_errors(&format!(
        r#"{STATIC_SCAFFOLD}
    public static function go(): void
    {{
        if (self::$repo === null) {{
            throw new \Exception('unset');
        }}

        self::takes(self::$repo);
    }}
}}
"#
    ));
}

#[test]
fn a_static_property_of_another_class_is_tracked_under_its_own_name() {
    assert_no_type_errors(
        r#"<?php
namespace Repro;

class Repo {}

class Holder {
    public static ?Repo $repo = null;
}

class Registry {
    public static function takes(Repo $r): void {}

    public static function go(): void
    {
        Holder::$repo = new Repo();
        self::takes(Holder::$repo);
    }
}
"#,
    );
}

#[test]
fn a_later_write_replaces_what_an_earlier_one_proved() {
    assert_type_error(&format!(
        r#"{STATIC_SCAFFOLD}
    public static function go(): void
    {{
        self::$repo = new Repo();
        self::$repo = null;
        self::takes(self::$repo);
    }}
}}
"#
    ));
}

#[test]
fn a_write_on_only_one_branch_proves_nothing_after_the_join() {
    assert_type_error(&format!(
        r#"{STATIC_SCAFFOLD}
    public static function go(bool $flag): void
    {{
        if ($flag) {{
            self::$repo = new Repo();
        }}

        self::takes(self::$repo);
    }}
}}
"#
    ));
}

#[test]
fn an_untouched_static_property_keeps_its_declared_type() {
    assert_type_error(
        r#"<?php
namespace Repro;

class Repo {}

class Holder {
    public static ?Repo $repo = null;
}

class Registry {
    public static function takes(Repo $r): void {}

    public static function go(): void
    {
        self::takes(Holder::$repo);
    }
}
"#,
    );
}

// ─── Assignments inside a `try` ─────────────────────────────────────────────

const TRY_SCAFFOLD: &str = r#"<?php
namespace Repro;

class Holder {}

class Runner {
    public function takes(Holder $h): void {}
"#;

#[test]
fn an_assignment_in_a_try_survives_a_catch_that_rethrows() {
    assert_no_type_errors(&format!(
        r#"{TRY_SCAFFOLD}
    public function go(?Holder $h): void
    {{
        if (!$h) {{
            try {{
                $h = new Holder();
            }} catch (\RuntimeException) {{
                throw new \LogicException('x');
            }}
        }}

        $this->takes($h);
    }}
}}
"#
    ));
}

#[test]
fn a_catch_that_falls_through_still_joins_its_state() {
    assert_type_error(&format!(
        r#"{TRY_SCAFFOLD}
    public function go(?Holder $h): void
    {{
        if (!$h) {{
            try {{
                $h = new Holder();
            }} catch (\RuntimeException) {{
                // Swallowed: `$h` is still null on this path.
            }}
        }}

        $this->takes($h);
    }}
}}
"#
    ));
}

// ─── `&&` chains in a match arm ─────────────────────────────────────────────

#[test]
fn an_and_chain_in_a_match_arm_narrows_its_own_operands() {
    assert_no_type_errors(
        r#"<?php
namespace Repro;

class Holder {}

class Runner {
    public ?Holder $a = null;
    public ?Holder $b = null;

    public function same(Holder $h): bool { return true; }

    public function go(int $kind): bool
    {
        return match ($kind) {
            1 => $this->a && $this->b && $this->same($this->a),
            default => true,
        };
    }
}
"#,
    );
}

// ─── Identity against an enum case ──────────────────────────────────────────

const ENUM_SCAFFOLD: &str = r#"<?php
namespace Repro;

enum Land {
    case Be;
    case Nl;
}

class Runner {
    public function takes(Land $land): bool { return true; }
"#;

#[test]
fn identity_with_an_enum_case_rules_out_null_for_the_rest_of_the_chain() {
    assert_no_type_errors(&format!(
        r#"{ENUM_SCAFFOLD}
    public function go(?Land $land): bool
    {{
        return $land === Land::Be && $this->takes($land);
    }}
}}
"#
    ));
}

#[test]
fn a_guard_on_the_negated_identity_proves_the_case_afterwards() {
    assert_no_type_errors(&format!(
        r#"{ENUM_SCAFFOLD}
    public function go(?Land $land): bool
    {{
        if ($land !== Land::Be) {{
            return false;
        }}

        return $this->takes($land);
    }}
}}
"#
    ));
}

#[test]
fn identity_with_a_constant_that_is_null_proves_nothing() {
    assert_type_error(
        r#"<?php
namespace Repro;

class Land {
    const NONE = null;
}

class Runner {
    public function takes(Land $land): bool { return true; }

    public function go(?Land $land): bool
    {
        return $land === Land::NONE && $this->takes($land);
    }
}
"#,
    );
}

// ─── Loop conditions ────────────────────────────────────────────────────────

const LOOP_SCAFFOLD: &str = r#"<?php
namespace Repro;

class Node {}

class Parser {
    public function parseOptional(): ?Node { return null; }

    public function addChild(array $list, Node $node): bool { return true; }
"#;

#[test]
fn a_do_while_condition_narrows_its_own_operands() {
    assert_no_type_errors(&format!(
        r#"{LOOP_SCAFFOLD}
    public function go(array $list): void
    {{
        do {{
            $node = $this->parseOptional();
        }} while ($node && $this->addChild($list, $node));
    }}
}}
"#
    ));
}

#[test]
fn a_for_condition_narrows_its_own_operands() {
    assert_no_type_errors(&format!(
        r#"{LOOP_SCAFFOLD}
    public function go(array $list): void
    {{
        for ($node = $this->parseOptional(); $node && $this->addChild($list, $node); ) {{
        }}
    }}
}}
"#
    ));
}

// ─── Unions with one class member ───────────────────────────────────────────

const MIXED_UNION_SCAFFOLD: &str = r#"<?php
namespace Repro;

class Decimal {
    public function format(): string { return ''; }
}

class Image {
    public function __construct(string $src) {}
    public function url(): string { return ''; }
}

function takesFloat(float $value): string { return (string) $value; }
"#;

#[test]
fn an_instanceof_else_keeps_the_scalar_half_of_the_union() {
    assert_no_type_errors(&format!(
        r#"{MIXED_UNION_SCAFFOLD}
function clip(Decimal|float $value): string
{{
    if ($value instanceof Decimal) {{
        return $value->format();
    }}
    return takesFloat($value);
}}
"#
    ));
}

#[test]
fn a_negated_instanceof_body_keeps_the_scalar_half_of_the_union() {
    assert_no_type_errors(&format!(
        r#"{MIXED_UNION_SCAFFOLD}
function imgix(Image|string $imgix): string
{{
    if (!$imgix instanceof Image) {{
        $imgix = new Image($imgix);
    }}
    return $imgix->url();
}}
"#
    ));
}

// ─── Repair inside the branch that detects the bad value ────────────────────

const REPAIR_SCAFFOLD: &str = r#"<?php
namespace Repro;

enum Status {
    case Active;
}

function takesString(string $s): string { return $s; }
function takesStatus(Status $s): void {}
"#;

#[test]
fn a_falsy_guard_that_repairs_the_value_merges_without_the_falsy_half() {
    assert_no_type_errors(&format!(
        r#"{REPAIR_SCAFFOLD}
function normalize(): string
{{
    $value = mb_strrchr('x', '\\');
    if (!$value) {{
        $value = 'fallback';
    }}
    return $value;
}}
"#
    ));
}

#[test]
fn an_is_array_guard_that_wraps_the_value_merges_to_an_array() {
    assert_no_type_errors(&format!(
        r#"{REPAIR_SCAFFOLD}
/** @param array<Status>|Status $status */
function toArray(array|Status $status): void
{{
    if (!is_array($status)) {{
        $status = [$status];
    }}
    foreach ($status as $s) {{
        takesStatus($s);
    }}
}}
"#
    ));
}

// ─── Reads that used to go back to the declaration ──────────────────────────
//
// A guard narrows the variable, then something *derived* from it is read:
// an array-dimension fetch, an array literal built around it, or an
// argument handed to one of the array functions whose result type follows
// its input. Each of those has its own resolution path, and each used to
// answer from the `@param`/`@var` the declaration states rather than from
// the scope the guard wrote.

const DERIVED_READ_SCAFFOLD: &str = r#"<?php
namespace Repro;

class Cache {
    public static function get(string $key): mixed { return null; }
}

function takesArgs(array $args): void {}
"#;

#[test]
fn an_is_array_guard_reaches_a_dimension_fetch() {
    assert_no_type_errors(&format!(
        r#"{DERIVED_READ_SCAFFOLD}
class Violation
{{
    /** @var array<int, string> */
    public array $args;

    /** @param array{{args: array<int, string>, message: string}}|string $violationMessage */
    public function __construct(array|string $violationMessage)
    {{
        if (is_array($violationMessage)) {{
            $this->args = $violationMessage['args'];
        }}
    }}
}}
"#
    ));
}

#[test]
fn a_not_null_guard_reaches_an_array_function_on_an_annotated_variable() {
    assert_no_type_errors(&format!(
        r#"{DERIVED_READ_SCAFFOLD}
class Versions
{{
    /** @return list<array{{version: string}}> */
    public function recent(int $limit): array
    {{
        /** @var null|list<array{{version: string}}> $cached */
        $cached = Cache::get('versions');
        if ($cached !== null) {{
            return array_slice($cached, 0, $limit);
        }}
        return [];
    }}
}}
"#
    ));
}

#[test]
fn a_phpstan_assert_reaches_a_dimension_fetch() {
    assert_no_type_errors(&format!(
        r#"{DERIVED_READ_SCAFFOLD}
class Asserts
{{
    /** @phpstan-assert !null $actual */
    public static function assertNotNull(mixed $actual): void {{}}
}}

class SectionTest extends Asserts
{{
    public function testSection(): void
    {{
        /** @var null|array{{categories: list<array{{id: int}}>}} $section */
        $section = Cache::get('section');
        self::assertNotNull($section);
        takesArgs($section['categories']);
    }}
}}
"#
    ));
}

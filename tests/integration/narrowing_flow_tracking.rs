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

// ─── A checked call is the same call where it is written again ──────────────

const REPEATED_CALL_SCAFFOLD: &str = r#"<?php
namespace Repro;

class User {}

class Session
{
    public static function current(): ?User { return null; }
}

function currentUser(): ?User { return null; }
function render(User $user): void {}
"#;

/// `if (currentUser())` proves the call's result non-null, and writing the
/// same call again inside the branch is the idiom the check exists for.
#[test]
fn a_guard_on_a_plain_function_call_narrows_the_repeated_call() {
    assert_no_type_errors(&format!(
        r#"{REPEATED_CALL_SCAFFOLD}
function show(): void
{{
    if (currentUser()) {{
        render(currentUser());
    }}
}}
"#
    ));
}

/// The same for a static call, whose key names the class rather than a
/// receiver variable.
#[test]
fn a_guard_on_a_static_call_narrows_the_repeated_call() {
    assert_no_type_errors(&format!(
        r#"{REPEATED_CALL_SCAFFOLD}
function show(): void
{{
    if (Session::current() !== null) {{
        render(Session::current());
    }}
}}
"#
    ));
}

/// Negative control: the guard says nothing about a *different* call, so
/// the nullable return survives.
#[test]
fn a_guard_on_one_call_leaves_another_alone() {
    assert_type_error(&format!(
        r#"{REPEATED_CALL_SCAFFOLD}
function show(): void
{{
    if (currentUser()) {{
        render(Session::current());
    }}
}}
"#
    ));
}

// ─── A `continue` guard reaches the copy the loop body makes of it ──────────

const CONTINUE_GUARD_SCAFFOLD: &str = r#"<?php
namespace Repro;

class Country {}

class Sheet
{
    /** @return array{0: false|string, 1: Country|false} */
    private function columnValues(string $key): array { return [false, false]; }

    private function updatePrice(int $productId, Country $market, array $row): void {}
"#;

/// `continue` on `!$newMarket` rules `false` out for the rest of the
/// iteration, so the copy two lines down only ever stores a `Country` —
/// including on the path where the copy is itself guarded and the merged
/// state is what the call reads.
#[test]
fn a_continue_guard_reaches_a_variable_the_value_is_copied_into() {
    assert_no_type_errors(&format!(
        r#"{CONTINUE_GUARD_SCAFFOLD}
    /** @param list<string> $keys */
    public function run(array $keys, array $row, int $productId): void
    {{
        $market = null;
        foreach ($keys as $key) {{
            [$dbCol, $newMarket] = $this->columnValues($key);
            if (!$dbCol || !$newMarket) {{
                continue;
            }}
            if (!$market) {{
                $market = $newMarket;
            }}
            $this->updatePrice($productId, $market, $row);
        }}
    }}
}}
"#
    ));
}

/// Negative control: without the guard, the `false` the shape declares is
/// still in play where the copy is read.
#[test]
fn an_unguarded_copy_keeps_the_falsy_union_member() {
    assert_type_error(&format!(
        r#"{CONTINUE_GUARD_SCAFFOLD}
    /** @param list<string> $keys */
    public function run(array $keys, array $row, int $productId): void
    {{
        $market = null;
        foreach ($keys as $key) {{
            [$dbCol, $newMarket] = $this->columnValues($key);
            if (!$market) {{
                $market = $newMarket;
            }}
            $this->updatePrice($productId, $market, $row);
        }}
    }}
}}
"#
    ));
}

// ─── An assertion against a class the subject does not implement ────────────

const MOCK_SCAFFOLD: &str = r#"<?php
namespace Repro;

interface MockObject {}
class MethodNode {}
class FunctionNode {}

class Asserts
{
    /**
     * @template ExpectedType of object
     * @param class-string<ExpectedType> $expected
     * @phpstan-assert =ExpectedType $actual
     */
    public static function assertInstanceOf(string $expected, mixed $actual): void {}
}
"#;

/// A mock really is both the interface it was built as and the class it
/// stands in for, so an assertion naming the class leaves an intersection
/// of the two. Recorded as a union instead, it satisfies neither half of
/// the declared `MethodNode&MockObject`.
#[test]
fn an_assertion_against_an_unrelated_class_intersects_rather_than_unions() {
    assert_no_type_errors(&format!(
        r#"{MOCK_SCAFFOLD}
class Probe extends Asserts
{{
    /** @return MethodNode&MockObject */
    protected function build(MockObject $node)
    {{
        static::assertInstanceOf(MethodNode::class, $node);

        return $node;
    }}
}}
"#
    ));
}

/// The same proof applied to a subject that is *already* an intersection
/// picks the union member it named and leaves the conjunct alone:
/// `(FunctionNode|MethodNode)&MockObject` proven `MethodNode` is
/// `MethodNode&MockObject`.
#[test]
fn an_assertion_picks_one_arm_of_a_union_inside_an_intersection() {
    assert_no_type_errors(&format!(
        r#"{MOCK_SCAFFOLD}
class Probe extends Asserts
{{
    /** @return (FunctionNode|MethodNode)&MockObject */
    private function make(string $class) {{ }}

    /** @return MethodNode&MockObject */
    protected function methodMock()
    {{
        $node = $this->make(MethodNode::class);
        static::assertInstanceOf(MethodNode::class, $node);

        return $node;
    }}

    /** @return FunctionNode&MockObject */
    protected function functionMock()
    {{
        $node = $this->make(FunctionNode::class);
        static::assertInstanceOf(FunctionNode::class, $node);

        return $node;
    }}
}}
"#
    ));
}

/// Negative control: the arm the assertion did *not* name is gone, so
/// returning the narrowed value as the other half is still a mismatch.
#[test]
fn an_assertion_rules_out_the_union_arm_it_did_not_name() {
    assert_type_error(&format!(
        r#"{MOCK_SCAFFOLD}
class Probe extends Asserts
{{
    /** @return (FunctionNode|MethodNode)&MockObject */
    private function make(string $class) {{ }}

    /** @return FunctionNode&MockObject */
    protected function methodMock()
    {{
        $node = $this->make(MethodNode::class);
        static::assertInstanceOf(MethodNode::class, $node);

        return $node;
    }}
}}
"#
    ));
}

// ─── A predicate that promises something about its own receiver ─────────────

const PREDICATE_SCAFFOLD: &str = r#"<?php
namespace PHPStan\Analyser;

class ClassReflection {}
class TraitReflection {}

interface Scope
{
    /** @phpstan-assert-if-true !null $this->getTraitReflection() */
    public function isInTrait(): bool;

    public function getTraitReflection(): ?TraitReflection;

    /** @api */
    public function isInClass(): bool;

    public function getClassReflection(): ?ClassReflection;
}

function useTrait(TraitReflection $reflection): void {}
function useClass(ClassReflection $reflection): void {}
"#;

/// `@phpstan-assert-if-true !null $this->getTraitReflection()` names a
/// member of the *receiver*, not a parameter, so the subject it narrows
/// is that member read through the variable the call was written on.
#[test]
fn a_predicate_narrows_the_member_its_tag_names_on_the_receiver() {
    assert_no_type_errors(&format!(
        r#"{PREDICATE_SCAFFOLD}
function f(Scope $scope): void
{{
    if ($scope->isInTrait()) {{
        $reflection = $scope->getTraitReflection();
        useTrait($reflection);
    }}
}}
"#
    ));
}

/// PHPStan annotates `isInTrait()` and leaves the identical `isInClass()`
/// bare, so the pairing is supplied for it. Every PHPStan extension is
/// written against it regardless of the missing tag.
#[test]
fn phpstan_is_in_class_narrows_the_paired_reflection_getter() {
    assert_no_type_errors(&format!(
        r#"{PREDICATE_SCAFFOLD}
function f(Scope $scope): void
{{
    if ($scope->isInClass()) {{
        $reflection = $scope->getClassReflection();
        useClass($reflection);
    }}
}}
"#
    ));
}

/// Negative control: without the guard the getter is as nullable as it
/// declares itself to be.
#[test]
fn an_unguarded_reflection_getter_keeps_its_null() {
    assert_type_error(&format!(
        r#"{PREDICATE_SCAFFOLD}
function f(Scope $scope): void
{{
    $reflection = $scope->getClassReflection();
    useClass($reflection);
}}
"#
    ));
}

/// A `!null` promise about a plain parameter narrows the same way — the
/// tag names no class, so it goes through the type guards rather than the
/// `instanceof` machinery.
#[test]
fn an_assert_if_true_not_null_tag_strips_the_null() {
    assert_no_type_errors(
        r#"<?php
namespace Repro;

class Row {}

class Reader
{
    /** @phpstan-assert-if-true !null $row */
    public function isLoaded(?Row $row): bool { return $row !== null; }
}

function takesRow(Row $row): void {}

function f(Reader $reader, ?Row $row): void
{
    if ($reader->isLoaded($row)) {
        takesRow($row);
    }
}
"#,
    );
}

// ─── A ternary that repeats its own subject ─────────────────────────────────

const SELF_TERNARY_SCAFFOLD: &str = r#"<?php
namespace Repro;

/**
 * @property null|string $alt
 * @property string $caption
 */
class Article
{
    public ?string $subtitle = null;
    public string $title = '';

    public function __get(string $name): mixed { return null; }
}

function takesString(string $value): void {}
"#;

/// `$a->alt ? $a->alt : $a->caption` proves the path truthy for its own
/// then-arm. The proof is keyed under the whole path, not under the
/// variable it is rooted at, which is why the arm has to look for it
/// there.
#[test]
fn a_self_referencing_ternary_narrows_a_property_path() {
    assert_no_type_errors(&format!(
        r#"{SELF_TERNARY_SCAFFOLD}
function render(Article $article): void
{{
    $alt = $article->alt ? $article->alt : $article->caption;
    takesString($alt);
}}
"#
    ));
}

/// The same for a real declared property, which took the same path and
/// lost the same proof.
#[test]
fn a_self_referencing_ternary_narrows_a_declared_property() {
    assert_no_type_errors(&format!(
        r#"{SELF_TERNARY_SCAFFOLD}
function render(Article $article): void
{{
    $subtitle = $article->subtitle ? $article->subtitle : $article->title;
    takesString($subtitle);
}}
"#
    ));
}

/// A `@var` block annotating the statement is authoritative over the
/// assignment it names and nothing else: the ternary in the same
/// statement still narrows.
#[test]
fn a_preceding_var_docblock_does_not_cancel_the_statements_narrowing() {
    assert_no_type_errors(&format!(
        r#"{SELF_TERNARY_SCAFFOLD}
function render(): void
{{
    /** @var Article $article */
    takesString($article->alt ? $article->alt : $article->caption);
}}
"#
    ));
}

/// Negative control: the else arm gets the opposite proof, so returning
/// the nullable half there is still a mismatch.
#[test]
fn the_else_arm_of_a_self_ternary_keeps_the_falsy_half() {
    assert_type_error(&format!(
        r#"{SELF_TERNARY_SCAFFOLD}
function render(Article $article): void
{{
    $alt = $article->caption ? $article->caption : $article->alt;
    takesString($alt);
}}
"#
    ));
}

// ─── Loops over an iterable that proves it has entries ──────────────────────

const FLOOR_SCAFFOLD: &str = r#"<?php
namespace Repro;

function takesInt(int $v): int { return $v; }
"#;

/// `if (!$qtys) { return; }` proves the array non-empty, so the body runs
/// at least once and the sentinel the loop was seeded with is gone by the
/// time the loop ends.
#[test]
fn a_guard_proving_the_iterable_non_empty_drops_the_pre_loop_sentinel() {
    assert_no_type_errors(&format!(
        r#"{FLOOR_SCAFFOLD}
/** @param array<int, int> $qtys */
function floorStock(array $qtys): int
{{
    if (!$qtys) {{
        return 0;
    }}
    $max = null;
    foreach ($qtys as $qty) {{
        if ($max === null) {{
            $max = $qty;
            continue;
        }}
        $max = min($max, $qty);
    }}
    return takesInt($max);
}}
"#
    ));
}

/// The same proof written into the parameter's own type rather than
/// carried in by a guard.
#[test]
fn a_non_empty_array_parameter_drops_the_pre_loop_sentinel() {
    assert_no_type_errors(&format!(
        r#"{FLOOR_SCAFFOLD}
/** @param non-empty-array<int, int> $qtys */
function floorStock(array $qtys): int
{{
    $max = null;
    foreach ($qtys as $qty) {{
        $max = $qty;
    }}
    return takesInt($max);
}}
"#
    ));
}

/// A shape with a required entry has at least that entry to iterate.
#[test]
fn an_array_shape_with_a_required_entry_drops_the_pre_loop_sentinel() {
    assert_no_type_errors(&format!(
        r#"{FLOOR_SCAFFOLD}
/** @param array{{first: int, second?: int}} $qtys */
function floorStock(array $qtys): int
{{
    $max = null;
    foreach ($qtys as $qty) {{
        $max = $qty;
    }}
    return takesInt($max);
}}
"#
    ));
}

/// Negative control: nothing proves this array has entries, so the loop
/// may not run and the sentinel survives it.
#[test]
fn an_unproven_iterable_keeps_the_pre_loop_sentinel() {
    assert_type_error(&format!(
        r#"{FLOOR_SCAFFOLD}
/** @param array<int, int> $qtys */
function floorStock(array $qtys): int
{{
    $max = null;
    foreach ($qtys as $qty) {{
        $max = $qty;
    }}
    return takesInt($max);
}}
"#
    ));
}

/// Negative control: an all-optional shape can be the empty array, so it
/// proves nothing about whether the body runs.
#[test]
fn an_all_optional_array_shape_keeps_the_pre_loop_sentinel() {
    assert_type_error(&format!(
        r#"{FLOOR_SCAFFOLD}
/** @param array{{first?: int}} $qtys */
function floorStock(array $qtys): int
{{
    $max = null;
    foreach ($qtys as $qty) {{
        $max = $qty;
    }}
    return takesInt($max);
}}
"#
    ));
}

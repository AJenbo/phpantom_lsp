<?php

// Source: Psalm TypeReconciliation/ConditionalTest.php
// Ported assertions that are novel (not already covered by existing tests).

use function PHPStan\Testing\assertType;

// ── arrayAssignmentPropagation ──────────────────────────────────────────
// After a conditional reassignment, the variable type should reflect
// both branches.
class ArrayAssignmentPropagation {
	public function test(): void {
		$dummy = ["test" => 123];

		/** @var array{test: ?int} */
		$a = ["test" => null];

		if ($a["test"] === null) {
			$a = $dummy;
		}
		$var = $a["test"];
		assertType('int', $var);
	}
}

// ── notInstanceOfPropertyElseif ─────────────────────────────────────────
// Chained elseif: is_string on property, then instanceof on property,
// else branch should have original type minus the narrowed-away types.
class NotInstanceOfPropertyElseif {
	public function test(): void {
		$a = new ElseifHolder();

		$out = null;

		if (is_string($a->foo)) {
			// $a->foo is string here
		}
		elseif ($a->foo instanceof ElseifChild) {
			// $a->foo is ElseifChild here
		}
		else {
			$out = $a->foo;
		}

		assertType('ElseifBase|null', $out);
	}
}

class ElseifBase {}
class ElseifChild extends ElseifBase {}

class ElseifHolder {
	/** @var string|ElseifBase */
	public $foo = "";
}

// ── ignoreNullCheckAndMaintainNullValue ──────────────────────────────────
// After a null-check branch that does NOT exit/return, the variable type
// should be unchanged (both branches rejoin).
class IgnoreNullCheck {
	public function testNull(): void {
		$a = null;
		if ($a !== null) { }
		$b = $a;
		assertType('null', $b);
	}

	public function testNullable(): void {
		$a = rand(0, 1) ? 5 : null;
		if ($a !== null) { }
		$b = $a;
		// PHPantom is more precise than Psalm here: it keeps the literal
		// `5` from the ternary rather than widening it to `int`.
		assertType('5|null', $b);
	}
}

// ── nullableIntReplacement ──────────────────────────────────────────────
// Complex OR condition with reassignment inside branch.
class NullableIntReplacement {
	public function test(): void {
		$a = rand(0, 1) ? 5 : null;

		$b = (bool)rand(0, 1);

		if ($b || $a !== null) {
			$a = 3;
		}

		// Either the guard fired and `$a` is `3`, or it did not — which
		// only happens when `$a` was `null` and `$b` was false, since the
		// `5` branch always makes `$a !== null` true on its own. So `5`
		// cannot survive to this merge.
		assertType('null|3', $a);
	}
}

// ── is_scalar narrowing ─────────────────────────────────────────────────
// is_scalar() should narrow string|null: true branch removes null,
// false branch removes string.
class IsScalarNarrowing {
	public function testRemoveStringWithIsScalar(): void {
		$a = rand(0, 1) ? "hello" : null;

		if (is_scalar($a)) {
			exit;
		}

		assertType('null', $a);
	}

	public function testRemoveNullWithIsScalar(): void {
		$a = rand(0, 1) ? "hello" : null;

		if (!is_scalar($a)) {
			exit;
		}

		// PHPantom is more precise than Psalm here: the surviving branch
		// keeps the literal `'hello'` rather than widening it to `string`.
		assertType('\'hello\'', $a);
	}
}

// ── classResolvesBackToSelfAfterComparison ──────────────────────────────
// After instanceof check + reassignment, the variable should resolve to
// the broader type (the parent).
class ClassResolvesBack {
	public function test(): void {
		$a = self::getA();
		if ($a instanceof ClassResolvesBackChild) {
			$a = new ClassResolvesBackChild;
		}

		assertType('ClassResolvesBack', $a);
	}

	public static function getA(): ClassResolvesBack {
		return new ClassResolvesBack();
	}
}

class ClassResolvesBackChild extends ClassResolvesBack {}

// ── is_numeric with exit narrows remaining type ─────────────────────────
class IsNumericNarrowing {
	public function test(): void {
		/** @var string|int $a */
		$a = rand(0, 5) > 4 ? "hello" : 5;

		if (is_numeric($a)) {
			exit;
		}

		assertType('string', $a);
	}
}

// ── is_bool with exit narrows remaining type ────────────────────────────
class IsBoolNarrowing {
	public function test(): void {
		/** @var string|bool $a */
		$a = rand(0, 5) > 4 ? "hello" : true;

		if (is_bool($a)) {
			exit;
		}

		assertType('string', $a);
	}
}

// ── short-circuited conditional ─────────────────────────────────────────
// After `if ($foo) {} elseif ($existing === null) { throw; }`,
// $existing is NOT narrowed because the first branch may have been taken.
class ShortCircuitedConditional {
	public function test(): void {
		/** @var ?stdClass $existing */
		$existing = null;

		/** @var bool $foo */
		$foo = true;

		if ($foo) {
		} elseif ($existing === null) {
			throw new \RuntimeException();
		}

		assertType('stdClass|null', $existing);
	}
}

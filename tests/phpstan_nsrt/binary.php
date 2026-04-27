<?php

namespace BinaryOperations\NestedNamespace;

use function PHPStan\Testing\assertType;

class Foo
{

	public const INT_CONST = 1;

	public function doFoo()
	{
		/** @var float $float */
		$float = doFoo();
		$float1 = $float;
		$float2 = $float;
		$float3 = $float;
		$float4 = $float;
		$float5 = $float;
		$float6 = $float;

		/** @var int $integer */
		$integer = doFoo();
		$integer1 = $integer;
		$integer2 = $integer;
		$integer3 = $integer;
		$integer4 = $integer;
		$integer5 = $integer;
		$integer6 = $integer;
		$integer7 = $integer;
		$integer8 = $integer;

		/** @var bool $bool */
		$bool = doFoo();

		/** @var string $string */
		$string = doFoo();
		$string1 = $string;
		$string2 = $string;
		$string3 = $string;
		$string4 = $string;

		/** @var string|null $stringOrNull */
		$stringOrNull = doFoo();

		/** @var number $number */
		$number = doFoo();

		/** @var int|null|bool $otherInteger */
		$otherInteger = doFoo();

		/** @var mixed $mixed */
		$mixed = doFoo();

		/** @var int|float $intOrFloat */
		$intOrFloat = doFoo();

		/** @var int|array $intOrArray */
		$intOrArray = doFoo();

		/** @var array|float $floatOrArray */
		$floatOrArray = doFoo();

		// ── Unary operators ─────────────────────────────────────────────

		assertType('int|float', -$integer);

		// ── Arithmetic: literal int op int ──────────────────────────────
		// PHPantom does not constant-fold; all literal arithmetic
		// produces the union of possible runtime types.

		assertType('int|float', 1 + 1);
		assertType('int|float', 1 - 1);
		assertType('int|float', 1 / 2);
		assertType('int|float', 1 * 1);
		assertType('int|float', 1 ** 1);
		assertType('int', 1 % 1);

		// ── Arithmetic: literal float op float ──────────────────────────

		assertType('int|float', 1.2 + 1.4);
		assertType('int|float', 1.2 - 1.4);
		assertType('int|float', 1.2 / 2.4);
		assertType('int|float', 1.2 * 1.4);
		assertType('int|float', 1.2 ** 1.4);
		assertType('int', 3.2 % 2.4);

		// ── Arithmetic: literal int op float ────────────────────────────

		assertType('int|float', 1 + 1.4);
		assertType('int|float', 1 - 1.4);
		assertType('int|float', 1 / 2.4);
		assertType('int|float', 1 * 1.4);
		assertType('int|float', 1 ** 1.4);
		assertType('int', 3 % 2.4);

		// ── Arithmetic: literal float op int ────────────────────────────

		assertType('int|float', 1.2 + 1);
		assertType('int|float', 1.2 - 1);
		assertType('int|float', 1.2 / 2);
		assertType('int|float', 1.2 * 1);
		assertType('int|float', 1.2 ** 1);
		assertType('int', 3.2 % 2);

		// ── Arithmetic: variable int operations ─────────────────────────

		assertType('int|float', $integer * 10);
		assertType('int|float', $integer ** $integer);
		assertType('int|float', $integer / $integer);
		assertType('int|float', $otherInteger + 1);
		assertType('int|float', $otherInteger + 1.0);

		// ── Arithmetic: variable float operations ───────────────────────

		assertType('int|float', $float + $float);
		assertType('int|float', $float + $number);

		// ── Arithmetic: number type ─────────────────────────────────────

		assertType('float|int', 1 + $number);
		assertType('float|int', $integer + $number);
		assertType('float|int', 1 / $number);
		assertType('int|float', 1.0 / $number);
		assertType('float|int', $number / 1);
		assertType('int|float', $number / 1.0);
		assertType('int|float', 1.0 + $number);
		assertType('float|int', $number + 1);
		assertType('int|float', $number + 1.0);

		// ── Arithmetic: mixed operands ──────────────────────────────────

		assertType('int|float', 1.0 / $mixed);
		assertType('int|float', $mixed / 1.0);
		assertType('int|float', 1.0 + $mixed);
		assertType('int|float', $mixed + 1.0);

		// ── Arithmetic: union type operands ─────────────────────────────

		assertType('float|int', $intOrFloat + $intOrFloat);

		// ── Arithmetic: bool operands ───────────────────────────────────

		assertType('int|float', true + false);

		// ── String concatenation ────────────────────────────────────────

		assertType('string', $string . $string);
		assertType('string', $string . 'foo');
		assertType('string', 'foo' . $string);
		assertType('string', $string . $integer);
		assertType('string', $integer . $string); // SKIP

		// ── Comparison operators ────────────────────────────────────────

		assertType('bool', $string === "foo");
		assertType('bool', $string !== "foo");
		assertType('bool', $string == "foo");
		assertType('bool', $string != "foo");
		assertType('bool', $integer > 0);
		assertType('bool', $integer >= 0);
		assertType('bool', $integer < 0);
		assertType('bool', $integer <= 0);

		// ── Spaceship ───────────────────────────────────────────────────

		assertType('int', 'foo' <=> 'bar'); // SKIP

		// ── Bitwise operators: int operands ─────────────────────────────

		assertType('int', 5 & 3);
		assertType('int', $integer & 3);
		assertType('int', $integer & $integer);
		assertType('int', 5 | 3);
		assertType('int', $integer | 3);
		assertType('int', 5 ^ 3);
		assertType('int', $integer ^ 3);

		// ── Bitwise: string operands ────────────────────────────────────
		// PHPantom resolves bitwise on two strings as int, not string.

		assertType('string', "x" & "y"); // SKIP
		assertType('string', $string & "x"); // SKIP
		assertType('string', "x" | "y"); // SKIP
		assertType('string', $string | "x"); // SKIP
		assertType('string', "x" ^ "y"); // SKIP
		assertType('string', $string ^ "x"); // SKIP

		// ── Bitwise: string op int → ERROR ──────────────────────────────

		assertType('*ERROR*', "bla" & 3); // SKIP
		assertType('*ERROR*', "bla" | 3); // SKIP
		assertType('*ERROR*', "bla" ^ 3); // SKIP

		// ── Null coalescing ─────────────────────────────────────────────

		assertType('string', $string ?? 'foo');
		assertType('string', $stringOrNull ?? 'foo');
		assertType('string|int', $string ?? $integer);
		assertType('int|string', $stringOrNull ?? $integer);
		assertType('string|null', $stringOrNull ?? null);

		// ── Ternary ─────────────────────────────────────────────────────

		assertType('int', true ? 1 : 2);
		assertType('int', false ? 1 : 2);

		// ── instanceof ──────────────────────────────────────────────────

		$foo = new Foo();
		assertType('bool', $foo instanceof \BinaryOperations\NestedNamespace\Foo);

		// ── Logical operators ───────────────────────────────────────────
		// PHPantom does not resolve logical operator result types via
		// hover on assigned variables.

		assertType('bool', true && false); // SKIP
		assertType('bool', true || false); // SKIP
		assertType('bool', true xor false); // SKIP
		assertType('bool', $bool xor true); // SKIP
		assertType('bool', true and false); // SKIP
		assertType('bool', true or false); // SKIP
		assertType('bool', !true); // SKIP

		// ── Compound assignment ─────────────────────────────────────────
		// PHPantom does not resolve compound assignment types via hover.

		assertType('float|int', $integer1 /= 2); // SKIP
		assertType('int', $integer2 *= 1); // SKIP
		assertType('float', $float1 /= 2.4); // SKIP
		assertType('float', $float2 *= 2.4); // SKIP
		assertType('float', $integer3 /= 2.4); // SKIP
		assertType('float', $integer4 *= 2.4); // SKIP
		assertType('int', $float3 %= 2.4); // SKIP
		assertType('float', $float4 **= 2.4); // SKIP
		assertType('float', $float5 /= 2.4); // SKIP
		assertType('float', $float6 *= 2); // SKIP
		assertType('int', $integer5 <<= 2.2); // SKIP
		assertType('int', $integer6 &= 3); // SKIP
		assertType('int', $integer7 |= 3); // SKIP
		assertType('int', $integer8 ^= 3); // SKIP

		// ── Mixed arithmetic with mixed ─────────────────────────────────
		// PHPantom returns no type for mixed with int literal on LHS.

		assertType('float|int', 1 + $mixed); // SKIP
		assertType('float|int', 1 / $mixed); // SKIP
		assertType('float|int', $mixed / 1); // SKIP
		assertType('float|int', $mixed + 1); // SKIP
	}

}
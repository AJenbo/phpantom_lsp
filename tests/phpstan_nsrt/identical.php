<?php

namespace TypeSpecifierIdentical;

use function PHPStan\Testing\assertType;

class Foo
{

	public function foo(\stdClass $a, \stdClass $b): void
	{
		if ($a === $a) {
			assertType('stdClass', $a);
		} else {
			assertType('*NEVER*', $a); // SKIP
		}

		if ($b !== $b) {
			assertType('*NEVER*', $b); // SKIP
		} else {
			assertType('stdClass', $b);
		}

		if ($a === $b) {
			assertType('stdClass', $a);
			assertType('stdClass', $b);
		} else {
			assertType('stdClass', $a);
			assertType('stdClass', $b);
		}

		if ($a !== $b) {
			assertType('stdClass', $a);
			assertType('stdClass', $b);
		} else {
			assertType('stdClass', $a);
			assertType('stdClass', $b);
		}

		assertType('stdClass', $a);
		assertType('stdClass', $b);
	}

	/**
	 * @param array{a: string, b: array{c: string|null}} $a
	 */
	public function arrayOffset(array $a): void
	{
		if (strlen($a['a']) > 0 && $a['a'] === $a['b']['c']) {
			assertType('non-empty-string', $a['a']); // SKIP
		}
	}

}

class Bar
{

	public function doFoo(\stdClass $a, \stdClass $b): void
	{
		assertType('true', $a === $a); // SKIP
		assertType('bool', $a === $b); // SKIP
		assertType('false', $a !== $a); // SKIP
		assertType('bool', $a !== $b); // SKIP
	}

	public static function createStdClass(): \stdClass
	{
		return new \stdClass();
	}

}

class NullNarrowing
{

	/**
	 * @param \stdClass|null $x
	 */
	public function equalsNull($x): void
	{
		if ($x === null) {
			assertType('null', $x);
		} else {
			assertType('stdClass', $x);
		}
	}

	/**
	 * @param \stdClass|null $x
	 */
	public function notEqualsNull($x): void
	{
		if ($x !== null) {
			assertType('stdClass', $x);
		} else {
			assertType('null', $x);
		}
	}

	/**
	 * @param \stdClass|null $x
	 */
	public function earlyReturnNull($x): void
	{
		if ($x === null) {
			return;
		}
		assertType('stdClass', $x);
	}

	/**
	 * @param \stdClass|null $x
	 */
	public function earlyReturnNotNull($x): void
	{
		if ($x !== null) {
			return;
		}
		assertType('null', $x);
	}

	/**
	 * @param string|null $s
	 */
	public function stringOrNull($s): void
	{
		if ($s === null) {
			assertType('null', $s);
		} else {
			assertType('string', $s);
		}
	}

	/**
	 * @param string|null $s
	 */
	public function stringNotNull($s): void
	{
		if ($s !== null) {
			assertType('string', $s);
		} else {
			assertType('null', $s);
		}
	}

	/**
	 * @param int|null $i
	 */
	public function intOrNull($i): void
	{
		if ($i === null) {
			assertType('null', $i);
		} else {
			assertType('int', $i);
		}
	}

	/**
	 * @param \stdClass|string|null $x
	 */
	public function tripleUnionNull($x): void
	{
		if ($x === null) {
			assertType('null', $x);
		} else {
			assertType('stdClass|string', $x);
		}
	}

	/**
	 * @param \stdClass|string|null $x
	 */
	public function tripleUnionNotNull($x): void
	{
		if ($x !== null) {
			assertType('stdClass|string', $x);
		} else {
			assertType('null', $x);
		}
	}

	public function nativeNullable(?\stdClass $x): void
	{
		if ($x === null) {
			assertType('null', $x);
		} else {
			assertType('stdClass', $x);
		}
	}

	public function nativeNullableNotEquals(?\stdClass $x): void
	{
		if ($x !== null) {
			assertType('stdClass', $x);
		} else {
			assertType('null', $x);
		}
	}

}
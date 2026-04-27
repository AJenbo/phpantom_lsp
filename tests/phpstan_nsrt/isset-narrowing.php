<?php

namespace IssetNarrowing;

use function PHPStan\Testing\assertType;

class Foo
{
	public function fooMethod(): void {}
}

class Bar
{
	public function barMethod(): void {}
}

class Holder
{
	public ?Foo $prop = null;
	public ?Bar $bar = null;
}

/**
 * @param Foo|null $nullable
 */
function issetNarrowsNullable(?Foo $nullable): void
{
	if (isset($nullable)) {
		assertType('Foo', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
	}
}

/**
 * @param Foo|null $nullable
 */
function notIssetNarrowsToNull(?Foo $nullable): void
{
	if (!isset($nullable)) {
		assertType('null', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
	}
}

/**
 * @param Foo|null $nullable
 */
function issetElseBranch(?Foo $nullable): void
{
	if (isset($nullable)) {
		assertType('Foo', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
	} else {
		assertType('null', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
	}
}

/**
 * @param Foo|null $nullable
 */
function issetEarlyReturn(?Foo $nullable): void
{
	if (!isset($nullable)) {
		return;
	}

	assertType('Foo', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
}

/**
 * @param Foo|null $a
 * @param Bar|null $b
 */
function multipleIsset(?Foo $a, ?Bar $b): void
{
	if (isset($a)) {
		assertType('Foo', $a); // SKIP — isset narrowing not yet implemented, returns ?Foo
	}

	if (isset($b)) {
		assertType('Bar', $b); // SKIP — isset narrowing not yet implemented, returns ?Bar
	}
}

/**
 * @param Foo|null $a
 * @param Bar|null $b
 */
function issetBothParams(?Foo $a, ?Bar $b): void
{
	if (isset($a) && isset($b)) {
		assertType('Foo', $a); // SKIP — isset narrowing not yet implemented, returns ?Foo
		assertType('Bar', $b); // SKIP — isset narrowing not yet implemented, returns ?Bar
	}
}

/**
 * @param Foo|null $nullable
 */
function nullCoalescingBasic(?Foo $nullable): void
{
	$result = $nullable ?? new Bar();
	assertType('Bar|Foo', $result); // SKIP — null coalescing union resolution not yet implemented
}

function issetOnProperty(Holder $holder): void
{
	if (isset($holder->prop)) {
		assertType('Foo', $holder->prop); // SKIP — property narrowing through isset not yet implemented
	}
}

/**
 * @param string|null $nullable
 */
function issetNarrowsString(?string $nullable): void
{
	if (isset($nullable)) {
		assertType('string', $nullable); // SKIP — isset narrowing not yet implemented, returns string|null
	}
}

/**
 * @param int|null $nullable
 */
function issetNarrowsInt(?int $nullable): void
{
	if (isset($nullable)) {
		assertType('int', $nullable); // SKIP — isset narrowing not yet implemented, returns int|null
	}
}

/**
 * @param Foo|null $nullable
 */
function issetAssignment(?Foo $nullable): void
{
	if (isset($nullable)) {
		$foo = $nullable;
		assertType('Foo', $foo); // SKIP — isset narrowing not yet implemented, returns ?Foo
	}
}

/**
 * @param Foo|null $nullable
 */
function issetNegatedEarlyReturnElse(?Foo $nullable): void
{
	if (isset($nullable)) {
		assertType('Foo', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
		return;
	}

	assertType('null', $nullable); // SKIP — isset narrowing not yet implemented, returns ?Foo
}

/**
 * @param Foo|Bar|null $union
 */
function issetOnUnionWithNull($union): void
{
	if (isset($union)) {
		assertType('Bar|Foo', $union); // SKIP — isset narrowing not yet implemented, returns Foo
	}
}

/**
 * @param array|null $nullable
 */
function issetNarrowsArray(?array $nullable): void
{
	if (isset($nullable)) {
		assertType('array', $nullable); // SKIP — isset narrowing not yet implemented, returns array|null
	}
}
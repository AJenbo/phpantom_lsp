<?php

namespace IsFunctionsNarrowing;

use function PHPStan\Testing\assertType;

/**
 * @param int|string $intOrString
 * @param int|float $intOrFloat
 * @param string|null $stringOrNull
 * @param int|string|null $intStringOrNull
 * @param object|string $objectOrString
 * @param array|string $arrayOrString
 * @param bool|int $boolOrInt
 */
function basicIsNarrowing(
	$intOrString,
	$intOrFloat,
	$stringOrNull,
	$intStringOrNull,
	$objectOrString,
	$arrayOrString,
	$boolOrInt
): void {
	if (is_int($intOrString)) {
		assertType('int', $intOrString);
	}

	if (is_string($intOrString)) {
		assertType('string', $intOrString);
	}

	if (is_int($intOrFloat)) {
		assertType('int', $intOrFloat);
	}

	if (is_float($intOrFloat)) {
		assertType('float', $intOrFloat);
	}

	if (is_string($stringOrNull)) {
		assertType('string', $stringOrNull);
	}

	if (is_null($stringOrNull)) {
		assertType('null', $stringOrNull);
	}

	if (is_int($intStringOrNull)) {
		assertType('int', $intStringOrNull);
	}

	if (is_string($intStringOrNull)) {
		assertType('string', $intStringOrNull);
	}

	if (is_null($intStringOrNull)) {
		assertType('null', $intStringOrNull);
	}

	if (is_object($objectOrString)) {
		assertType('object', $objectOrString);
	}

	if (is_string($objectOrString)) {
		assertType('string', $objectOrString);
	}

	if (is_array($arrayOrString)) {
		assertType('array', $arrayOrString);
	}

	if (is_string($arrayOrString)) {
		assertType('string', $arrayOrString);
	}

	if (is_bool($boolOrInt)) {
		assertType('bool', $boolOrInt);
	}

	if (is_int($boolOrInt)) {
		assertType('int', $boolOrInt);
	}
}

/**
 * @param int|string|null $value
 */
function negatedIsNarrowing($value): void
{
	if (!is_int($value)) {
		assertType('string|null', $value);
	}

	if (!is_string($value)) {
		assertType('int|null', $value);
	}

	if (!is_null($value)) {
		assertType('int|string', $value);
	}
}

/**
 * @param int|string|null $value
 */
function earlyReturnIsNarrowing($value): void
{
	if (is_null($value)) {
		return;
	}

	assertType('int|string', $value);
}

/**
 * @param int|string|float $value
 */
function chainedIsNarrowing($value): void
{
	if (is_int($value)) {
		assertType('int', $value);
	} elseif (is_string($value)) {
		assertType('string', $value);
	} else {
		assertType('float', $value);
	}
}

class Foo
{
	public function fooMethod(): void {}
}

class Bar
{
	public function barMethod(): void {}
}

/**
 * @param Foo|Bar|int $value
 */
function isObjectNarrowsUnion($value): void
{
	if (is_object($value)) {
		assertType('Foo|Bar', $value);
	}

	if (is_int($value)) {
		assertType('int', $value);
	}
}

/**
 * @param int|string $value
 */
function isNumericDoesNotNarrow($value): void
{
	// is_numeric doesn't narrow the type in a union of int|string
	// because both int and string can be numeric.
	if (is_int($value)) {
		assertType('int', $value);
	}
}

/**
 * @param int|string|null $value
 */
function earlyReturnNegatedIsNarrowing($value): void
{
	if (!is_string($value)) {
		return;
	}

	assertType('string', $value);
}

/**
 * @param Foo|string|null $value
 */
function multipleGuards($value): void
{
	if (is_null($value)) {
		return;
	}

	if (is_string($value)) {
		return;
	}

	assertType('Foo', $value);
}

/**
 * @param Foo|int|string|null $value
 */
function isObjectEarlyReturn($value): void
{
	if (!is_object($value)) {
		return;
	}

	assertType('Foo', $value);
}
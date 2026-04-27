<?php

namespace InstanceOfNamespace;

use function PHPStan\Testing\assertType;

interface BarInterface
{
}

class BarParent
{
}

class Foo extends BarParent implements BarInterface
{
	public function fooMethod(): void {}
}

class Bar
{
	public function barMethod(): void {}
}

class Baz
{
	public function bazMethod(): void {}
}

/**
 * @param BarParent $parent
 * @param BarInterface $iface
 * @param Foo|Bar $union
 * @param Foo|Bar|Baz $tripleUnion
 * @param Foo|null $nullable
 * @param mixed $mixed
 * @param object $object
 */
function basicInstanceof(
	BarParent $parent,
	BarInterface $iface,
	$union,
	$tripleUnion,
	?Foo $nullable,
	$mixed,
	object $object
): void {
	// Simple instanceof on a union narrows to the matching branch.
	if ($union instanceof Foo) {
		assertType('Foo', $union);
	}

	if ($union instanceof Bar) {
		assertType('Bar', $union);
	}

	// instanceof on a triple union narrows to the matching type.
	if ($tripleUnion instanceof Foo) {
		assertType('Foo', $tripleUnion);
	}

	if ($tripleUnion instanceof Bar) {
		assertType('Bar', $tripleUnion);
	}

	if ($tripleUnion instanceof Baz) {
		assertType('Baz', $tripleUnion);
	}

	// instanceof on nullable strips null.
	if ($nullable instanceof Foo) {
		assertType('Foo', $nullable);
	}

	// instanceof on mixed narrows to the checked type.
	if ($mixed instanceof Foo) {
		assertType('Foo', $mixed);
	}

	if ($mixed instanceof BarInterface) {
		assertType('BarInterface', $mixed);
	}

	// instanceof on object narrows to the checked type.
	if ($object instanceof Foo) {
		assertType('Foo', $object);
	}
}

/**
 * @param Foo|Bar|null $value
 */
function negatedInstanceof($value): void
{
	if (!$value instanceof Foo) {
		// After negation, Foo is removed from the union.
		assertType('Bar|null', $value);
	}
}

/**
 * @param Foo|Bar|Baz $value
 */
function chainedInstanceof($value): void
{
	if ($value instanceof Foo) {
		assertType('Foo', $value);
	} elseif ($value instanceof Bar) {
		assertType('Bar', $value);
	} else {
		assertType('Baz', $value);
	}
}

/**
 * @param Foo|Bar|null $value
 */
function instanceofEarlyReturn($value): void
{
	if (!$value instanceof Foo) {
		return;
	}

	// After early return, value is narrowed to Foo.
	assertType('Foo', $value);
}

class SelfAndParentInstanceof extends BarParent
{
	/**
	 * @param BarParent|Bar $value
	 */
	public function testSelf($value): void
	{
		if ($value instanceof self) {
			assertType('SelfAndParentInstanceof', $value);
		}
	}
}

/**
 * @param Foo|Bar $union
 */
function instanceofInWhile($union): void
{
	if ($union instanceof Foo) {
		$x = $union;
		assertType('Foo', $x);
	}
}

class InstanceofWithAssignment
{
	/**
	 * @param Foo|Bar $value
	 */
	public function test($value): void
	{
		if ($value instanceof Foo) {
			$foo = $value;
			assertType('Foo', $foo);
		}

		if ($value instanceof Bar) {
			$bar = $value;
			assertType('Bar', $bar);
		}
	}
}
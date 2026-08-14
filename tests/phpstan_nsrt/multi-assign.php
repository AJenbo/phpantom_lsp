<?php

namespace MultiAssign;

use function PHPStan\Testing\assertType;

class Foo
{
	public function fooMethod(): void {}
}

class Bar
{
	public function barMethod(): void {}
}

// Chain assignments ($a = $b = expr) are not yet resolved by PHPantom's
// variable resolution pipeline. All assertions below are SKIP until
// chain assignment support is implemented.

function multiAssignNull(): void {
	$foo = $bar = $baz = null;
	assertType('null', $foo);
	assertType('null', $bar);
	assertType('null', $baz);
}

function multiAssignInt(): void {
	$a = $b = $c = 42;
	assertType('42', $a);
	assertType('42', $b);
	assertType('42', $c);
}

function multiAssignString(): void {
	$a = $b = 'hello';
	assertType('\'hello\'', $a);
	assertType('\'hello\'', $b);
}

function multiAssignFloat(): void {
	$a = $b = 3.14;
	assertType('3.14', $a);
	assertType('3.14', $b);
}

function multiAssignBool(): void {
	$a = $b = true;
	assertType('true', $a);
	assertType('true', $b);
}

function multiAssignObject(): void {
	$a = $b = new Foo();
	assertType('Foo', $a);
	assertType('Foo', $b);
}

function multiAssignFromParam(int $x): void {
	$a = $b = $x;
	assertType('int', $a);
	assertType('int', $b);
}

/**
 * @param Foo|Bar $union
 */
function multiAssignUnion($union): void {
	$a = $b = $union;
	assertType('Foo|Bar', $a);
	assertType('Foo|Bar', $b);
}

function reassignAfterChain(): void {
	$a = $b = 1;
	assertType('1', $a);
	assertType('1', $b);

	$a = 'changed';
	assertType('\'changed\'', $a);
	assertType('1', $b);
}

function multiAssignArray(): void {
	$a = $b = [1, 2, 3];
	assertType('array{1, 2, 3}', $a);
	assertType('array{1, 2, 3}', $b);
}

/**
 * @param string|null $nullable
 */
function multiAssignNullable(?string $nullable): void {
	$a = $b = $nullable;
	assertType('string|null', $a);
	assertType('string|null', $b);
}

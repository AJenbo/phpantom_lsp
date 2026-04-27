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
	assertType('null', $foo); // SKIP
	assertType('null', $bar); // SKIP
	assertType('null', $baz); // SKIP
}

function multiAssignInt(): void {
	$a = $b = $c = 42;
	assertType('int', $a); // SKIP
	assertType('int', $b); // SKIP
	assertType('int', $c); // SKIP
}

function multiAssignString(): void {
	$a = $b = 'hello';
	assertType('string', $a); // SKIP
	assertType('string', $b); // SKIP
}

function multiAssignFloat(): void {
	$a = $b = 3.14;
	assertType('float', $a); // SKIP
	assertType('float', $b); // SKIP
}

function multiAssignBool(): void {
	$a = $b = true;
	assertType('bool', $a); // SKIP
	assertType('bool', $b); // SKIP
}

function multiAssignObject(): void {
	$a = $b = new Foo();
	assertType('Foo', $a); // SKIP
	assertType('Foo', $b); // SKIP
}

function multiAssignFromParam(int $x): void {
	$a = $b = $x;
	assertType('int', $a); // SKIP
	assertType('int', $b); // SKIP
}

/**
 * @param Foo|Bar $union
 */
function multiAssignUnion($union): void {
	$a = $b = $union;
	assertType('Foo|Bar', $a); // SKIP
	assertType('Foo|Bar', $b); // SKIP
}

function reassignAfterChain(): void {
	$a = $b = 1;
	assertType('int', $a); // SKIP
	assertType('int', $b); // SKIP

	$a = 'changed';
	assertType('string', $a);
	assertType('int', $b); // SKIP
}

function multiAssignArray(): void {
	$a = $b = [1, 2, 3];
	assertType('array', $a); // SKIP
	assertType('array', $b); // SKIP
}

/**
 * @param string|null $nullable
 */
function multiAssignNullable(?string $nullable): void {
	$a = $b = $nullable;
	assertType('string|null', $a); // SKIP
	assertType('string|null', $b); // SKIP
}
<?php

namespace ClassConstantTypes;

use function PHPStan\Testing\assertType;

class Foo
{

	const NO_TYPE = 1;

	/** @var string */
	const TYPE = 'foo';

	/** @var string */
	private const PRIVATE_TYPE = 'foo';

	const FLOAT_CONST = 3.14;

	const BOOL_CONST = true;

	const NULL_CONST = null;

	const ARRAY_CONST = [1, 2, 3];

	public function doFoo()
	{
		assertType('1', self::NO_TYPE);
		assertType("'foo'", self::TYPE);
		assertType("'foo'", self::PRIVATE_TYPE);
		assertType('3.14', self::FLOAT_CONST);
		assertType('bool', self::BOOL_CONST);
		assertType('null', self::NULL_CONST);
		assertType('array', self::ARRAY_CONST);
	}

}

class Bar extends Foo
{

	const TYPE = 'bar';

	private const PRIVATE_TYPE = 'bar';

	const EXTRA = 99;

	public function doFoo()
	{
		assertType("'bar'", self::TYPE);
		assertType("'bar'", self::PRIVATE_TYPE);
		assertType('99', self::EXTRA);

		assertType('1', self::NO_TYPE);
		assertType('3.14', self::FLOAT_CONST);
		assertType('bool', self::BOOL_CONST);
		assertType('null', self::NULL_CONST);
	}

}

class Baz extends Foo
{

	/** @var int */
	const TYPE = 1;

	public function doFoo()
	{
		assertType('1', self::TYPE);

		assertType('1', self::NO_TYPE);
		assertType('3.14', self::FLOAT_CONST);
	}

}

final class FinalFoo
{

	const NO_TYPE = 1;

	/** @var string */
	const TYPE = 'foo';

	/** @var string */
	private const PRIVATE_TYPE = 'foo';

	public function doFoo()
	{
		assertType('1', self::NO_TYPE);
		assertType("'foo'", self::TYPE);
		assertType("'foo'", self::PRIVATE_TYPE);
	}

}

class ConstantExpressions
{

	const A = 10;
	const B = 20;
	const STR_A = 'hello';
	const STR_B = 'world';
	const FLOAT_A = 1.5;
	const BOOL_A = false;

	public function doFoo()
	{
		assertType('10', self::A);
		assertType('20', self::B);
		assertType("'hello'", self::STR_A);
		assertType("'world'", self::STR_B);
		assertType('1.5', self::FLOAT_A);
		assertType('bool', self::BOOL_A);
	}

}

class InheritedConstants extends Foo
{

	public function accessInherited()
	{
		assertType('1', self::NO_TYPE);
		assertType("'foo'", self::TYPE);
		assertType('3.14', self::FLOAT_CONST);
		assertType('bool', self::BOOL_CONST);
		assertType('null', self::NULL_CONST);
		assertType('array', self::ARRAY_CONST);
	}

}
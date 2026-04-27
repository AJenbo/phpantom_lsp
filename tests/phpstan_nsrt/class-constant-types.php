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
		assertType('int', self::NO_TYPE);
		assertType('string', self::TYPE);
		assertType('string', self::PRIVATE_TYPE);
		assertType('float', self::FLOAT_CONST);
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
		assertType('string', self::TYPE);
		assertType('string', self::PRIVATE_TYPE);
		assertType('int', self::EXTRA);

		assertType('int', self::NO_TYPE); // SKIP — inherited constant access not resolved
		assertType('float', self::FLOAT_CONST); // SKIP
		assertType('bool', self::BOOL_CONST); // SKIP
		assertType('null', self::NULL_CONST); // SKIP
	}

}

class Baz extends Foo
{

	/** @var int */
	const TYPE = 1;

	public function doFoo()
	{
		assertType('int', self::TYPE);

		assertType('int', self::NO_TYPE); // SKIP — inherited constant access not resolved
		assertType('float', self::FLOAT_CONST); // SKIP
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
		assertType('int', self::NO_TYPE);
		assertType('string', self::TYPE);
		assertType('string', self::PRIVATE_TYPE);
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
		assertType('int', self::A);
		assertType('int', self::B);
		assertType('string', self::STR_A);
		assertType('string', self::STR_B);
		assertType('float', self::FLOAT_A);
		assertType('bool', self::BOOL_A);
	}

}

class InheritedConstants extends Foo
{

	public function accessInherited()
	{
		assertType('int', self::NO_TYPE); // SKIP — inherited constant access not resolved
		assertType('string', self::TYPE); // SKIP
		assertType('float', self::FLOAT_CONST); // SKIP
		assertType('bool', self::BOOL_CONST); // SKIP
		assertType('null', self::NULL_CONST); // SKIP
		assertType('array', self::ARRAY_CONST); // SKIP
	}

}
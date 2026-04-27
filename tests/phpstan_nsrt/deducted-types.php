<?php

namespace TypesNamespaceDeductedTypes;

use function PHPStan\Testing\assertType;

class Lorem
{
}

final class Foo
{

	const INTEGER_CONSTANT = 1;
	const FLOAT_CONSTANT = 1.0;
	const STRING_CONSTANT = 'foo';
	const ARRAY_CONSTANT = [];
	const BOOLEAN_CONSTANT = true;
	const NULL_CONSTANT = null;

	public function doFoo(): void
	{
		$integerLiteral = 1;
		$booleanLiteral = true;
		$anotherBooleanLiteral = false;
		$stringLiteral = 'foo';
		$floatLiteral = 1.0;
		$nullLiteral = null;
		$loremObjectLiteral = new Lorem();
		$foo = new self();
		$arrayLiteral = [];

		assertType('int', $integerLiteral);
		assertType('bool', $booleanLiteral);
		assertType('bool', $anotherBooleanLiteral);
		assertType('string', $stringLiteral);
		assertType('float', $floatLiteral);
		assertType('null', $nullLiteral);
		assertType('Lorem', $loremObjectLiteral);
		assertType('Foo', $foo);
		assertType('array', $arrayLiteral);
	}

	public function classConstants(): void
	{
		assertType('int', self::INTEGER_CONSTANT);
		assertType('float', self::FLOAT_CONSTANT);
		assertType('string', self::STRING_CONSTANT);
		assertType('array', self::ARRAY_CONSTANT);
		assertType('bool', self::BOOLEAN_CONSTANT);
		assertType('null', self::NULL_CONSTANT);


	}

	public function newExpression(): void
	{
		$lorem = new Lorem();
		assertType('Lorem', $lorem);

		$foo = new Foo();
		assertType('Foo', $foo);

		$self = new self();
		assertType('Foo', $self);
	}

	public function reassignment(): void
	{
		$x = 1;
		assertType('int', $x);

		$x = 'hello';
		assertType('string', $x);

		$x = new Lorem();
		assertType('Lorem', $x);

		$x = null;
		assertType('null', $x);
	}

	/**
	 * @param int $intParam
	 * @param string $strParam
	 * @param float $floatParam
	 * @param bool $boolParam
	 */
	public function parameterTypes(
		int $intParam,
		string $strParam,
		float $floatParam,
		bool $boolParam
	): void
	{
		assertType('int', $intParam);
		assertType('string', $strParam);
		assertType('float', $floatParam);
		assertType('bool', $boolParam);
	}

	/**
	 * @param int|string $union
	 * @param Foo|null $nullable
	 */
	public function unionTypes($union, ?Foo $nullable): void
	{
		assertType('int|string', $union);
		assertType('Foo|null', $nullable);
	}
}
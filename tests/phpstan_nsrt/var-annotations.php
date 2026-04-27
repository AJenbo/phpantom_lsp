<?php

namespace VarAnnotations;

use function PHPStan\Testing\assertType;

class Lorem {}

class Foo
{

	public function doFoo()
	{
		/** @var int $integer */
		$integer = getFoo();

		/** @var bool $boolean */
		$boolean = getFoo();

		/** @var string $string */
		$string = getFoo();

		/** @var float $float */
		$float = getFoo();

		/** @var Lorem $loremObject */
		$loremObject = getFoo();

		/** @var mixed $mixed */
		$mixed = getFoo();

		/** @var array $array */
		$array = getFoo();

		/** @var bool|null $isNullable */
		$isNullable = getFoo();

		/** @var self $self */
		$self = getFoo();

		/** @var int $invalidInt */
		$invalidInteger = $this->getFloat();

		/** @var static $static */
		$static = getFoo();

		assertType('int', $integer);
		assertType('bool', $boolean);
		assertType('string', $string);
		assertType('float', $float);
		assertType('VarAnnotations\Lorem', $loremObject);
		assertType('mixed', $mixed);
		assertType('array', $array);
		assertType('bool|null', $isNullable);
		assertType('VarAnnotations\Foo', $self);
		assertType('float', $invalidInteger);
		// SKIP: PHPantom does not resolve static(ClassName) syntax
		// assertType('static(VarAnnotations\Foo)', $static);
	}

	public function doFooBar()
	{
		/** @var int */
		$integer = getFoo();

		/** @var bool */
		$boolean = getFoo();

		/** @var string */
		$string = getFoo();

		/** @var float */
		$float = getFoo();

		/** @var Lorem */
		$loremObject = getFoo();

		/** @var mixed */
		$mixed = getFoo();

		/** @var array */
		$array = getFoo();

		/** @var bool|null */
		$isNullable = getFoo();

		/** @var self */
		$self = getFoo();

		/** @var float */
		$invalidInteger = 1.0;

		assertType('int', $integer);
		assertType('bool', $boolean);
		assertType('string', $string);
		assertType('float', $float);
		assertType('VarAnnotations\Lorem', $loremObject);
		assertType('mixed', $mixed);
		assertType('array', $array);
		assertType('bool|null', $isNullable);
		assertType('VarAnnotations\Foo', $self);
		assertType('float', $invalidInteger);
	}

	public function getFloat(): float
	{
		return 1.0;
	}

}
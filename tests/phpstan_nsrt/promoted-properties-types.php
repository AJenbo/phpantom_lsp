<?php

namespace PromotedPropertiesTypes;

use function PHPStan\Testing\assertType;

/**
 * @template T
 */
class Foo
{

	/**
	 * @param array<int, string> $anotherPhpDocArray
	 * @param T $anotherTemplateProperty
	 * @param string $bothProperty
	 * @param array<string> $anotherBothProperty
	 */
	public function __construct(
		public $noType,
		public int $nativeIntType,
		/** @var array<int, string> */ public $phpDocArray,
		public $anotherPhpDocArray,
		/** @var array<int, string> */ public array $yetAnotherPhpDocArray,
		/** @var T */ public $templateProperty,
		public $anotherTemplateProperty,
		/** @var int */ public $bothProperty,
		/** @var array<int> */ public $anotherBothProperty
	) {
		assertType('array<int, string>', $phpDocArray);
		assertType('array<int, string>', $anotherPhpDocArray);
		assertType('array<int, string>', $yetAnotherPhpDocArray);
		assertType('int', $bothProperty);
		assertType('array<int>', $anotherBothProperty);
	}

}

function (Foo $foo): void {
	assertType('mixed', $foo->noType);
	assertType('int', $foo->nativeIntType);
	assertType('array<int, string>', $foo->phpDocArray);
	assertType('array<int, string>', $foo->anotherPhpDocArray);
	assertType('array<int, string>', $foo->yetAnotherPhpDocArray);
	assertType('int', $foo->bothProperty);
	assertType('array<int>', $foo->anotherBothProperty);
};

/**
 * @extends Foo<\stdClass>
 */
class Bar extends Foo
{

}

function (Bar $bar): void {
	assertType('stdClass', $bar->templateProperty);
	assertType('stdClass', $bar->anotherTemplateProperty);
};

/**
 * @template T
 */
class Lorem
{

	/**
	 * @param T $foo
	 */
	public function __construct(
		public $foo
	) { }

}

// SKIP: PHPantom does not infer constructor generic argument types
// function (): void {
// 	$lorem = new Lorem(new \stdClass);
// 	assertType('stdClass', $lorem->foo);
// };

/**
 * @extends Foo<\stdClass>
 */
class Baz extends Foo
{

	public function __construct(
		public $anotherPhpDocArray
	)
	{
		assertType('array<int, string>', $anotherPhpDocArray);
	}

}

function (Baz $baz): void {
	assertType('array<int, string>', $baz->anotherPhpDocArray);
	assertType('stdClass', $baz->templateProperty);
};

class PromotedPropertyNotNullable
{

	public function __construct(
		public int $intProp = null,
	) {}

}

function (PromotedPropertyNotNullable $p) {
	assertType('int', $p->intProp);
};
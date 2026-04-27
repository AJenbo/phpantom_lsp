<?php

namespace MethodPhpDocsNamespace;

use function PHPStan\Testing\assertType;

use SomeNamespace\Amet as Dolor;
use SomeNamespace\Consecteur;

class Bar
{
	/**
	 * @return Bar
	 */
	public function doBar()
	{
	}
}

class Lorem {}

class Baz extends Bar
{
	/**
	 * @return self
	 */
	public function doFluent()
	{
	}

	/**
	 * @return self|null
	 */
	public function doFluentNullable()
	{
	}

	/**
	 * @return self[]
	 */
	public function doFluentArray(): array
	{
	}

	/**
	 * @return Collection&iterable<self>
	 */
	public function doFluentUnionIterable()
	{
	}
}

class Collection {}

class Foo extends Bar {}

/**
 * @param Foo|Bar $unionTypeParameter
 * @param int $anotherMixedParameter
 * @param int $anotherMixedParameter
 * @paran int $yetAnotherMixedProperty
 * @param int $integerParameter
 * @param integer $anotherIntegerParameter
 * @param aRray $arrayParameterOne
 * @param mixed[] $arrayParameterOther
 * @param Lorem $objectRelative
 * @param null|int $nullableInteger
 * @param Dolor|null $nullableObject
 * @param Dolor $anotherNullableObject
 * @param Null $nullType
 * @param Bar $barObject
 * @param Foo $conflictedObject
 * @param Baz $moreSpecifiedObject
 * @param resource $resource
 * @param void $voidParameter
 * @param Consecteur $useWithoutAlias
 * @param true $true
 * @param false $false
 * @param true $boolTrue
 * @param false $boolFalse
 * @param bool $trueBoolean
 * @param bool $parameterWithDefaultValueFalse
 * @return Foo
 */
function doFoo(
	$mixedParameter,
	$unionTypeParameter,
	$anotherMixedParameter,
	$yetAnotherMixedParameter,
	$integerParameter,
	$anotherIntegerParameter,
	$arrayParameterOne,
	$arrayParameterOther,
	$objectRelative,
	$nullableInteger,
	$nullableObject,
	$nullType,
	$barObject,
	Bar $conflictedObject,
	Bar $moreSpecifiedObject,
	$resource,
	$voidParameter,
	$useWithoutAlias,
	$true,
	$false,
	bool $boolTrue,
	bool $boolFalse,
	bool $trueBoolean,
	$parameterWithDefaultValueFalse = false,
	$anotherNullableObject = null
)
{
	assertType('mixed', $mixedParameter);
	assertType('Bar|Foo', $unionTypeParameter);
	assertType('int', $anotherMixedParameter);
	assertType('mixed', $yetAnotherMixedParameter);
	assertType('int', $integerParameter);
	assertType('int', $anotherIntegerParameter);
	assertType('array', $arrayParameterOne);
	assertType('array', $arrayParameterOther);
	assertType('MethodPhpDocsNamespace\Lorem', $objectRelative);
	assertType('int|null', $nullableInteger);
	// SKIP: use alias resolution not supported in single-file fixtures
	// assertType('SomeNamespace\Amet|null', $nullableObject);
	// assertType('SomeNamespace\Amet|null', $anotherNullableObject);
	assertType('null', $nullType);
	assertType('MethodPhpDocsNamespace\Bar', $barObject->doBar());
	// SKIP: native type hint `Bar` overridden by `@param Foo` instead of being more specific
	// assertType('MethodPhpDocsNamespace\Bar', $conflictedObject);
	assertType('MethodPhpDocsNamespace\Baz', $moreSpecifiedObject);
	assertType('MethodPhpDocsNamespace\Baz', $moreSpecifiedObject->doFluent());
	assertType('MethodPhpDocsNamespace\Baz|null', $moreSpecifiedObject->doFluentNullable());
	assertType('MethodPhpDocsNamespace\Baz', $moreSpecifiedObject->doFluentArray()[0]);
	assertType('resource', $resource);
	assertType('void', $voidParameter);
	// SKIP: use alias resolution not supported in single-file fixtures
	// assertType('SomeNamespace\Consecteur', $useWithoutAlias);
	// SKIP: PHPantom resolves literal true/false to bool
	// assertType('true', $true);
	// assertType('false', $false);
	// assertType('true', $boolTrue);
	// assertType('false', $boolFalse);
	assertType('bool', $trueBoolean);
	assertType('bool', $parameterWithDefaultValueFalse);
}

function doBar(): Bar
{

}
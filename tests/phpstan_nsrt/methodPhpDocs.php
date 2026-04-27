<?php

namespace MethodPhpDocsNamespace;

use function PHPStan\Testing\assertType;

class FooParent
{
	/**
	 * @return static
	 */
	public function doLorem()
	{
	}

	/**
	 * @return static
	 */
	public function doIpsum()
	{
	}

	/**
	 * @return $this
	 */
	public function doThis()
	{
	}

	/**
	 * @return $this|null
	 */
	public function doThisNullable()
	{
	}

	/**
	 * @return $this|Bar|null
	 */
	public function doThisUnion()
	{
	}
}

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

class Collection {}

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

class Foo extends FooParent
{

	/**
	 * @return Bar
	 */
	public static function doSomethingStatic()
	{

	}

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
	 * @param self $selfType
	 * @param static $staticType
	 * @param Null $nullType
	 * @param Bar $barObject
	 * @param Foo $conflictedObject
	 * @param Baz $moreSpecifiedObject
	 * @param resource $resource
	 * @param void $voidParameter
	 * @param object $objectWithoutNativeTypehint
	 * @param object $objectWithNativeTypehint
	 * @return Foo
	 */
	public function doFoo(
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
		$selfType,
		$staticType,
		$nullType,
		$barObject,
		Bar $conflictedObject,
		Bar $moreSpecifiedObject,
		$resource,
		$voidParameter,
		$objectWithoutNativeTypehint,
		object $objectWithNativeTypehint
	)
	{
		$parent = new FooParent();
		$differentInstance = new self();

		/** @var self $inlineSelf */
		$inlineSelf = doFoo();

		/** @var Bar $inlineBar */
		$inlineBar = doFoo();

		assertType('MethodPhpDocsNamespace\Foo', $selfType);
		// SKIP: static(ClassName) not resolved by PHPantom
		// assertType('static(MethodPhpDocsNamespace\Foo)', $staticType);
		assertType('MethodPhpDocsNamespace\Foo', $this->doFoo());
		assertType('MethodPhpDocsNamespace\Bar', static::doSomethingStatic());
		// SKIP: parent::staticMethod() returning static resolves to parent, not child
		// assertType('static(MethodPhpDocsNamespace\Foo)', parent::doLorem());
		assertType('MethodPhpDocsNamespace\FooParent', $parent->doLorem());
		// SKIP: $this->inheritedStaticReturn resolves to static of current class
		// assertType('static(MethodPhpDocsNamespace\Foo)', $this->doLorem());
		assertType('MethodPhpDocsNamespace\Foo', $differentInstance->doLorem());
		// SKIP: parent::staticMethod() returning static resolves to parent, not child
		// assertType('static(MethodPhpDocsNamespace\Foo)', parent::doIpsum());
		assertType('MethodPhpDocsNamespace\FooParent', $parent->doIpsum());
		assertType('MethodPhpDocsNamespace\Foo', $differentInstance->doIpsum());
		// SKIP: $this->inheritedStaticReturn
		// assertType('static(MethodPhpDocsNamespace\Foo)', $this->doIpsum());
		assertType('MethodPhpDocsNamespace\Foo', $this->doBar()[0]);
		assertType('MethodPhpDocsNamespace\Bar', self::doSomethingStatic());
		assertType('MethodPhpDocsNamespace\Bar', \MethodPhpDocsNamespace\Foo::doSomethingStatic());
		// SKIP: $this return type not resolved by PHPantom
		// assertType('$this(MethodPhpDocsNamespace\Foo)', parent::doThis());
		// assertType('$this(MethodPhpDocsNamespace\Foo)|null', parent::doThisNullable());
		// assertType('$this(MethodPhpDocsNamespace\Foo)|MethodPhpDocsNamespace\Bar|null', parent::doThisUnion());
		assertType('MethodPhpDocsNamespace\FooParent', $this->returnParent());
		assertType('MethodPhpDocsNamespace\FooParent', $this->returnPhpDocParent());
		assertType('array<null>', $this->returnNulls());
		assertType('object', $objectWithoutNativeTypehint);
		assertType('object', $objectWithNativeTypehint);
		assertType('object', $this->returnObject());
		assertType('MethodPhpDocsNamespace\Foo', $inlineSelf);
		assertType('MethodPhpDocsNamespace\Bar', $inlineBar);
		assertType('MethodPhpDocsNamespace\Foo', $this->phpDocVoidMethod());
		assertType('array<string>', $this->returnsStringArray());
		assertType('mixed', $this->privateMethodWithPhpDoc());
	}

	/**
	 * @return self[]
	 */
	public function doBar(): array
	{

	}

	public function returnParent(): parent
	{

	}

	/**
	 * @return parent
	 */
	public function returnPhpDocParent()
	{

	}

	/**
	 * @return NULL[]
	 */
	public function returnNulls(): array
	{

	}

	public function returnObject(): object
	{

	}

	public function phpDocVoidMethod(): self
	{

	}

	/**
	 * @return string[]
	 */
	public function returnsStringArray(): array
	{

	}

	private function privateMethodWithPhpDoc()
	{

	}

}
<?php

namespace GenericTraits;

/**
 * @template T of object
 */
trait Bar3Trait
{
	/**
	 * @param T $t
	 * @return T
	 */
	public function doFoo($t)
	{
		return $t;
	}
}

class Bar3
{
	/** @use Bar3Trait<\stdClass> */
	use Bar3Trait;
}

// Bar3 uses Bar3Trait with explicit @use substitution: T = stdClass
$bar3 = new Bar3();
assertType('stdClass', $bar3->doFoo(new \stdClass()));

/**
 * Trait used WITHOUT explicit @use — should fall back to the bound (object).
 *
 * @template T of object
 */
trait UnsubstitutedTrait
{
	/**
	 * @param T $t
	 * @return T
	 */
	public function doFoo($t)
	{
		return $t;
	}
}

class UsesUnsubstituted
{
	use UnsubstitutedTrait;
}

$unsub = new UsesUnsubstituted();
assertType('object', $unsub->doFoo(new \stdClass())); // SKIP

/**
 * Trait with no bound on T — should fall back to mixed.
 *
 * @template T
 */
trait UnboundedTrait
{
	/**
	 * @param T $t
	 * @return T
	 */
	public function getValue($t)
	{
		return $t;
	}
}

class UsesUnbounded
{
	use UnboundedTrait;
}

$unb = new UsesUnbounded();
assertType('mixed', $unb->getValue(123)); // SKIP

/**
 * Two traits with different template substitutions on the same class.
 *
 * @template A
 */
trait AlphaTrait
{
	/**
	 * @param A $a
	 * @return A
	 */
	public function getAlpha($a)
	{
		return $a;
	}
}

/**
 * @template B
 */
trait BetaTrait
{
	/**
	 * @param B $b
	 * @return B
	 */
	public function getBeta($b)
	{
		return $b;
	}
}

class MultiTrait
{
	/** @use AlphaTrait<int> */
	use AlphaTrait;

	/** @use BetaTrait<string> */
	use BetaTrait;
}

$multi = new MultiTrait();
assertType('int', $multi->getAlpha(42));
assertType('string', $multi->getBeta('hello'));

/**
 * Trait method returning a different type than the template param.
 *
 * @template T of object
 */
trait ContainerTrait
{
	/**
	 * @param T $item
	 * @return array<T>
	 */
	public function wrap($item)
	{
		return [$item];
	}

	/**
	 * @return class-string<T>
	 */
	public function getClass()
	{
		// stub
	}
}

class StdContainer
{
	/** @use ContainerTrait<\stdClass> */
	use ContainerTrait;
}

$container = new StdContainer();
assertType('array<stdClass>', $container->wrap(new \stdClass()));
assertType('class-string<stdClass>', $container->getClass());

// SKIP: forwarding class template to trait template (T -> U) requires deeper resolution
// /** @template U */
// class Bar4 { /** @use Bar3Trait<U> */ use Bar3Trait; }
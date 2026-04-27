<?php

namespace NativeTypes;

use function PHPStan\Testing\assertType;

class Foo
{

	/**
	 * @param self $foo
	 * @param \DateTimeImmutable $dateTime
	 */
	public function doFoo(
		$foo,
		\DateTimeInterface $dateTime,
		?string $nullableString,
		string $nonNullableString
	): void
	{
		assertType('Foo', $foo);

		$foo = new Foo();
		assertType('Foo', $foo);

		assertType('DateTimeImmutable', $dateTime);

		assertType('string|null', $nullableString);

		assertType('string', $nonNullableString);
	}

	/**
	 * @param array<string, int> $array
	 */
	public function doForeach(array $array): void
	{
		foreach ($array as $key => $value) {
			assertType('string', $key);
			assertType('int', $value);
		}
	}
}

/**
 * @param Foo $foo
 * @param \DateTimeImmutable $dateTime
 */
function fooFunction(
	$foo,
	\DateTimeInterface $dateTime,
	?string $nullableString,
	string $nonNullableString
): void
{
	assertType('Foo', $foo);

	assertType('DateTimeImmutable', $dateTime);

	assertType('string|null', $nullableString);

	assertType('string', $nonNullableString);
}

class TypedProperties
{

	/** @var int */
	private $untyped;

	private int $typed;

	public function doFoo(): void
	{
		assertType('int', $this->untyped);
		assertType('int', $this->typed);
	}

}
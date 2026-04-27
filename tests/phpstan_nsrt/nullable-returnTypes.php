<?php

namespace NullableReturnTypes;

use function PHPStan\Testing\assertType;

class Foo
{

	public function doFoo(): ?int
	{
		assertType('int|null', $this->doFoo());
		assertType('int|null', $this->doBar());
		assertType('int|null', $this->doConflictingNullable());
		assertType('int', $this->doAnotherConflictingNullable());
	}

	/**
	 * @return int|null
	 */
	public function doBar(): ?int
	{

	}

	/**
	 * @return int
	 */
	public function doConflictingNullable(): ?int
	{

	}

	/**
	 * @return int|null
	 */
	public function doAnotherConflictingNullable(): int
	{

	}

}
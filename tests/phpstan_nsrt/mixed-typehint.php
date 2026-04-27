<?php // lint >= 8.0

namespace MixedTypehint;

use function PHPStan\Testing\assertType;

class Foo
{

	public function doFoo(mixed $foo)
	{
		assertType('mixed', $foo);
		assertType('mixed', $this->doBar());
	}

	public function doBar(): mixed
	{

	}

}
<?php

namespace ArrayShapesInPhpDoc;

use function PHPStan\Testing\assertType;

class Bar {}
class Baz {}

class Foo
{

	/**
	 * @param array{0: string, 1: Foo, foo: Bar, Baz} $one
	 * @param array{0: string, 1?: Foo, foo?: Bar} $two
	 * @param array{0?: string, 1?: Foo, foo?: Bar} $three
	 */
	public function doFoo(
		array $one,
		array $two,
		array $three
	)
	{
		assertType('array{0: string, 1: Foo, foo: Bar, Baz}', $one);
		assertType('array{0: string, 1?: Foo, foo?: Bar}', $two);
		assertType('array{0?: string, 1?: Foo, foo?: Bar}', $three);
	}

	/**
	 * @param array{name: string, age: int} $person
	 */
	public function doBar(array $person)
	{
		assertType('string', $person['name']);
		assertType('int', $person['age']);
	}

	/**
	 * @param array{id: int, tags: array<string>} $item
	 */
	public function nestedTypes(array $item)
	{
		assertType('int', $item['id']);
		assertType('array<string>', $item['tags']);
	}

	/**
	 * @return array{success: bool, message: string}
	 */
	public function returnShape(): array
	{
		return ['success' => true, 'message' => 'ok'];
	}

	public function testReturnShape()
	{
		$result = $this->returnShape();
		assertType('bool', $result['success']);
		assertType('string', $result['message']);
	}

	/**
	 * @param array{inner: array{x: int, y: int}} $nested
	 */
	public function nestedShape(array $nested)
	{
		assertType('array{x: int, y: int}', $nested['inner']);
		assertType('int', $nested['inner']['x']);
		assertType('int', $nested['inner']['y']);
	}

	/**
	 * @param array{foo: Foo, bar: Bar} $objects
	 */
	public function objectValues(array $objects)
	{
		assertType('Foo', $objects['foo']);
		assertType('Bar', $objects['bar']);
	}

	/**
	 * @param array{nullable?: string} $opt
	 */
	public function optionalKey(array $opt)
	{
		
		// assertType('string', $opt['nullable']);
	}


	/**
	 * The empty shape has no entry any key could address, so an offset
	 * read on it is a guaranteed miss.
	 *
	 * @param array<string, int> $ints
	 */
	public function emptyShapeOffsets(array $ints, string $k, int $n)
	{
		$empty = [];
		assertType('null', $empty['missing']);
		assertType('0', $empty[$k] ?? 0);
		assertType('int', ($empty[$k] ?? 0) + $n);

		$totals = [];
		foreach ($ints as $key => $value) {
			$totals[$key] = ($totals[$key] ?? 0) + $value;
		}
		assertType('array<string, int>', $totals);
	}

	/**
	 * `array{}` is the empty array, which every array alternative that
	 * demands no entry already contains, so it drops out of the join.
	 *
	 * @param list<string> $names
	 */
	public function emptyShapeAbsorbedByArraySibling(array $names, bool $c)
	{
		$collected = [];
		$append = function (string $name) use (&$collected): void {
			$collected[] = $name;
		};
		assertType('list<string>', $collected);
		assertType('string', $collected[0]);

		$maybe = [];
		if ($c) {
			$maybe = $names;
		}
		assertType('list<string>', $maybe);
	}

}

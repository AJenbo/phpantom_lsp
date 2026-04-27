<?php

namespace ArrayDestructuring;

use function PHPStan\Testing\assertType;

class Foo
{

	public function mixedDestructuring(): void
	{
		/** @var mixed $array */
		$array = $this->getMixed();
		[$a, $b] = $array;
		list($aList, $bList) = $array;

		assertType('mixed', $a);
		assertType('mixed', $b);
		assertType('mixed', $aList);
		assertType('mixed', $bList);
	}

	public function stringArrayDestructuring(): void
	{
		/** @var string[] $stringArray */
		$stringArray = $this->getStringArray();
		[$first, $second] = $stringArray;
		assertType('string', $first);
		assertType('string', $second);
	}

	public function foreachDestructuring(): void
	{
		/** @var array<int, array{string, int}> $rows */
		$rows = $this->getRows();
		foreach ($rows as [$name, $age]) {
			assertType('string', $name);
			assertType('int', $age);
		}
	}

	public function foreachListDestructuring(): void
	{
		/** @var array<int, array{string, int}> $rows */
		$rows = $this->getRows();
		foreach ($rows as list($name, $age)) {
			assertType('string', $name);
			assertType('int', $age);
		}
	}

	public function shapeDestructuring(): void
	{
		/** @var array{name: string, age: int, active: bool} $person */
		$person = $this->getPerson();
		['name' => $name, 'age' => $age, 'active' => $active] = $person;
		assertType('string', $name);
		assertType('int', $age);
		assertType('bool', $active);
	}

	public function nestedDestructuring(): void
	{
		/** @var array{string, array{int, bool}} $nested */
		$nested = $this->getNested();
		[$str, [$num, $flag]] = $nested;
		assertType('string', $str);
		assertType('int', $num);
		assertType('bool', $flag);
	}

	public function foreachShapeDestructuring(): void
	{
		/** @var array<int, array{id: int, label: string}> $items */
		$items = $this->getItems();
		foreach ($items as ['id' => $id, 'label' => $label]) {
			assertType('int', $id);
			assertType('string', $label);
		}
	}

	/**
	 * @param array{x: float, y: float, z: float} $point
	 */
	public function paramDestructuring(array $point): void
	{
		['x' => $x, 'y' => $y, 'z' => $z] = $point;
		assertType('float', $x);
		assertType('float', $y);
		assertType('float', $z);
	}

	/**
	 * @param array<int, array{name: string, score: int}> $players
	 */
	public function foreachNestedShapeDestructuring(array $players): void
	{
		foreach ($players as ['name' => $name, 'score' => $score]) {
			assertType('string', $name);
			assertType('int', $score);
		}
	}

	public function varAnnotationAfterDestructuring(): void
	{
		/** @var string $stringWithVarAnnotation */
		[$stringWithVarAnnotation] = $this->getMixed();

		assertType('string', $stringWithVarAnnotation);
	}

	/** @return mixed */
	private function getMixed() { return null; }

}
<?php
// Source: Psalm Template/FunctionClassStringTemplateTest.php
// Auto-extracted by scripts/extract_psalm_tests.php
// Do not edit manually — re-run the extraction script instead.

// Test: returnTemplatedClassClassName
namespace PsalmTest_template_function_class_string_template_1 {
    class I {
        /**
         * @template T as Foo
         * @param class-string<T> $class
         * @return T|null
         */
        public function loader(string $class) {
            return $class::load();
        }
    }

    /**
     * @psalm-consistent-constructor
     */
    class Foo {
        /** @return static */
        public static function load() {
            return new static();
        }
    }

    class FooChild extends Foo{}

    $a = (new I)->loader(FooChild::class);

    assertType('FooChild|null', $a);
}

// Test: templateFilterArrayWithIntersection
namespace PsalmTest_template_function_class_string_template_2 {
    /**
     * @template T as object
     * @template S as object
     * @param array<T> $a
     * @param interface-string<S> $type
     * @return array<T&S>
     */
    function filter(array $a, string $type): array {
        $result = [];
        foreach ($a as $item) {
            if (is_a($item, $type)) {
                $result[] = $item;
            }
        }
        return $result;
    }

    interface A {}
    interface B {}

    /** @var array<A> */
    $x = [];
    $y = filter($x, B::class);

    assertType('array<A&B>', $y);
}

// Test: templateFilterWithIntersection
namespace PsalmTest_template_function_class_string_template_3 {
    /**
     * @template T as object
     * @template S as object
     * @param T $item
     * @param interface-string<S> $type
     * @return T&S
     */
    function filter($item, string $type) {
        if (is_a($item, $type)) {
            return $item;
        };

        throw new \UnexpectedValueException("bad");
    }

    interface A {}
    interface B {}

    /** @var A */
    $x = null;

    $y = filter($x, B::class);

    assertType('A&B', $y); 
}

// Test: templateFromDifferentClassStrings
namespace PsalmTest_template_function_class_string_template_4 {
    /**
     * @psalm-consistent-constructor
     */
    class A {}

    class B extends A {}
    class C extends A {}

    /**
     * @template T of A
     * @param class-string<T> $a1
     * @param class-string<T> $a2
     * @return T
     */
    function test(string $a1, string $a2) {
        if (rand(0, 1)) return new $a1();

        return new $a2();
    }

    $b_or_c = test(B::class, C::class);

    assertType('B|C', $b_or_c);
}


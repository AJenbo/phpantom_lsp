<?php
// Source: Extracted from Psalm Template/ClassTemplateTest.php
// Focused test for union generic method resolution without multi-namespace conflicts.

namespace PsalmTest_union_generic_method {
    class A {}
    class B {}

    /**
     * @template T
     */
    class C {
        /**
         * @var T
         */
        private $t;

        /**
         * @param T $t
         */
        public function __construct($t) {
            $this->t = $t;
        }

        /**
         * @return T
         */
        public function get() {
            return $this->t;
        }
    }

    /**
     * @param C<A> $a
     * @param C<B> $b
     * @return C<A>|C<B>
     */
    function randomCollection(C $a, C $b) : C {
        if (rand(0, 1)) {
            return $a;
        }

        return $b;
    }

    $random_collection = randomCollection(new C(new A), new C(new B));

    $a_or_b = $random_collection->get();

    assertType('C<A>|C<B>', $random_collection);
    assertType('A|B', $a_or_b);

    // Also test with @var annotation directly
    /** @var C<A>|C<B> $var_annotated */
    $var_annotated_result = $var_annotated->get();

    assertType('C<A>|C<B>', $var_annotated);
    assertType('A|B', $var_annotated_result);

    // Test with constructor inference and union
    $ca = new C(new A);
    $cb = new C(new B);

    assertType('C<A>', $ca);
    assertType('C<B>', $cb);
    assertType('A', $ca->get());
    assertType('B', $cb->get());

    // Test template bound used as default when no constructor args
    /**
     * @template T2 of object
     */
    final class Bounded {}

    $bounded = new Bounded();

    assertType('Bounded<object>', $bounded);

    // Test empty array inferred as never for template params
    /**
     * @template TKey of array-key
     * @template TVal
     */
    class TypedCollection {
        /** @var array<TKey, TVal> */
        private $items;

        /** @param array<TKey, TVal> $items */
        public function __construct(array $items = []) {
            $this->items = $items;
        }
    }

    $empty = new TypedCollection([]);

    assertType('TypedCollection<never, never>', $empty);

    // Test static method with method-level template returning generic
    /**
     * @template T
     */
    class GenericBox {
        /** @var array<T> */
        protected $items = [];

        /** @param array<string, T> $items */
        public function __construct(array $items = []) {
            $this->items = $items;
        }

        /**
         * @template C as object
         * @param class-string<C> $classString
         * @param array<string, C> $elements
         * @return GenericBox<C>
         */
        public static function fromClassString(string $classString, array $elements = []) : GenericBox {
            return new GenericBox($elements);
        }
    }

    $packages = GenericBox::fromClassString(A::class);

    assertType('GenericBox<A>', $packages);
}
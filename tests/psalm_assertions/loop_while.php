<?php
// Source: Psalm Loop/WhileTest.php
// Auto-extracted by scripts/extract_psalm_tests.php
// Do not edit manually — re-run the extraction script instead.

// Test: whileVar
namespace PsalmTest_loop_while_1 {
    $worked = false;

    while (rand(0,100) === 10) {
        $worked = true;
    }

    assertType('bool', $worked);
}

// Test: objectValueWithTwoTypes
namespace PsalmTest_loop_while_2 {
    class B {}
    class A {
        /** @var A|B */
        public $parent;

        public function __construct() {
            $this->parent = rand(0, 1) ? new A() : new B();
        }
    }

    function makeA(): A {
        return new A();
    }

    $a = makeA();

    while ($a instanceof A) {
        $a = $a->parent;
    }

    assertType('B', $a);
}

// Test: objectValueWithInstanceofProperty
namespace PsalmTest_loop_while_3 {
    class B {}
    class A {
        /** @var A|B */
        public $parent;

        public function __construct() {
            $this->parent = rand(0, 1) ? new A() : new B();
        }
    }

    function makeA(): A {
        return new A();
    }

    $a = makeA();

    while ($a->parent instanceof A) {
        $a = $a->parent;
    }

    $b = $a->parent;

    assertType('A', $a);
    assertType('A|B', $b);
}

// Test: objectValueNullable
namespace PsalmTest_loop_while_4 {
    class A {
        /** @var ?A */
        public $parent;

        public function __construct() {
            $this->parent = rand(0, 1) ? new A() : null;
        }
    }

    function makeA(): A {
        return new A();
    }

    $a = makeA();

    while ($a) {
        $a = $a->parent;
    }

    assertType('null', $a);
}

// Test: objectValueWithAnd
namespace PsalmTest_loop_while_5 {
    class A {
        /** @var ?A */
        public $parent;

        public function __construct() {
            $this->parent = rand(0, 1) ? new A() : null;
        }
    }

    function makeA(): A {
        return new A();
    }

    $a = makeA();

    while ($a && rand(0, 10) > 5) {
        $a = $a->parent;
    }

    assertType('A|null', $a);
}

// Test: whileTrueWithBreak
namespace PsalmTest_loop_while_6 {
    while (true) {
        $a = "hello";
        break;
    }
    while (1) {
        $b = 5;
        break;
    }

    assertType('string', $a);
    assertType('int', $b);
}

// Test: whileWithNotEmptyCheck
namespace PsalmTest_loop_while_7 {
    class A {
      /** @var A|null */
      public $a;

      public function __construct() {
        $this->a = rand(0, 1) ? new A : null;
      }
    }

    function takesA(A $a): void {}

    $a = new A();
    while ($a) {
      takesA($a);
      $a = $a->a;
    };

    assertType('null', $a);
}


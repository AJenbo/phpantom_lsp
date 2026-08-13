<?php
// Source: Psalm Loop/DoTest.php
// Auto-extracted by scripts/extract_psalm_tests.php
// Do not edit manually — re-run the extraction script instead.

// Test: doWhileVarWithPossibleBreak
namespace PsalmTest_loop_do_1 {
    $a = false;

    do {
        if (rand(0, 1)) {
            break;
        }
        if (rand(0, 1)) {
            $a = true;
            break;
        }
        $a = true;
    }
    while (rand(0,100) === 10);

    // SKIP: a `break` that leaves the loop before the assignment does not
    // contribute its state to the post-loop merge, so the pre-loop `false`
    // is lost. Tracked in docs/todo/bugs.md.
    assertType('bool', $a); // SKIP
}

// Test: doWhileVarWithPossibleBreakThatMaybeSetsToTrue
namespace PsalmTest_loop_do_2 {
    $a = false;

    do {
        if (rand(0, 1)) {
            if (rand(0, 1)) {
                $a = true;
            }

            break;
        }
        $a = true;
    }
    while (rand(0,1));

    // SKIP: a `break` that leaves the loop before the assignment does not
    // contribute its state to the post-loop merge, so the pre-loop `false`
    // is lost. Tracked in docs/todo/bugs.md.
    assertType('bool', $a); // SKIP
}

// Test: doWhileWithNotEmptyCheck
namespace PsalmTest_loop_do_3 {
    class A {
        /** @var A|null */
        public $a;

        public function __construct() {
            $this->a = rand(0, 1) ? new A : null;
        }
    }

    function takesA(A $a): void {}

    $a = new A();
    do {
        takesA($a);
        $a = $a->a;
    } while ($a);

    assertType('null', $a);
}

// Test: doWhileWithMethodCall
namespace PsalmTest_loop_do_4 {
    class A {
        public function getParent(): ?A {
            return rand(0, 1) ? new A() : null;
        }
    }

    $a = new A();

    do {
        $a = $a->getParent();
    } while ($a);

    assertType('null', $a);
}


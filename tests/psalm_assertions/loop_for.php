<?php
// Source: Psalm Loop/ForTest.php
// Auto-extracted by scripts/extract_psalm_tests.php
// Do not edit manually — re-run the extraction script instead.

// Test: falseToBoolInContinueAndBreak
namespace PsalmTest_loop_for_1 {
    $a = false;

    for ($i = 0; $i < 4; $i++) {
      $j = rand(0, 10);

      if ($j === 2) {
        $a = true;
        continue;
      }

      if ($j === 3) {
        $a = true;
        break;
      }
    }

    assertType('bool', $a);
}

// Test: whileTrueWithBreak
namespace PsalmTest_loop_for_2 {
    for (;;) {
        $a = "hello";
        break;
    }
    for (;;) {
        $b = 5;
        break;
    }

    // PHPantom is more precise than Psalm here: the single assignment
    // reaching each break keeps its literal value.
    assertType('\'hello\'', $a);
    assertType('5', $b);
}


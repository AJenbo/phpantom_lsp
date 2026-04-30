<?php
// Source: Psalm Loop/ForeachTest.php
// Auto-extracted by scripts/extract_psalm_tests.php
// Do not edit manually — re-run the extraction script instead.

// Test: switchVariableWithFallthrough
namespace PsalmTest_loop_foreach_1 {
    foreach (["a", "b", "c"] as $letter) {
        switch ($letter) {
            case "a":
            case "b":
                $foo = 2;
                break;

            default:
                $foo = 3;
                break;
        }

        $moo = $foo;
    }

    assertType('int', $moo);
}

// Test: switchVariableWithFallthroughStatement
namespace PsalmTest_loop_foreach_2 {
    foreach (["a", "b", "c"] as $letter) {
        switch ($letter) {
            case "a":
                $bar = 1;

            case "b":
                $foo = 2;
                break;

            default:
                $foo = 3;
                break;
        }

        $moo = $foo;
    }

    assertType('int', $moo);
}

// Test: assignInsideForeach
namespace PsalmTest_loop_foreach_3 {
    $b = false;

    foreach ([1, 2, 3, 4] as $a) {
        if ($a === rand(0, 10)) {
            $b = true;
        }
    }

    assertType('bool', $b);
}

// Test: assignInsideForeachWithBreak
namespace PsalmTest_loop_foreach_4 {
    $b = false;

    foreach ([1, 2, 3, 4] as $a) {
        if ($a === rand(0, 10)) {
            $b = true;
            break;
        }
    }

    assertType('bool', $b);
}

// Test: bleedVarIntoOuterContextWithEmptyLoop
namespace PsalmTest_loop_foreach_5 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
    }

    assertType('string', $tag);
}

// Test: bleedVarIntoOuterContextWithRedefinedAsNull
namespace PsalmTest_loop_foreach_6 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $tag = null;
      } else {
        $tag = null;
      }
    }

    assertType('null', $tag);
}

// Test: bleedVarIntoOuterContextWithRedefinedAsNullAndBreak
namespace PsalmTest_loop_foreach_7 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $tag = null;
        break;
      } elseif ($tag === "b") {
        $tag = null;
        break;
      } else {
        $tag = null;
        break;
      }
    }

    assertType('null', $tag);
}

// Test: bleedVarIntoOuterContextWithBreakInElse
namespace PsalmTest_loop_foreach_8 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $tag = null;
      } else {
        break;
      }
    }

    assertType('null|string', $tag);
}

// Test: bleedVarIntoOuterContextWithBreakInIf
namespace PsalmTest_loop_foreach_9 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        break;
      } else {
        $tag = null;
      }
    }

    assertType('null|string', $tag);
}

// Test: bleedVarIntoOuterContextWithBreakInElseAndIntSet
namespace PsalmTest_loop_foreach_10 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $tag = 5;
      } else {
        break;
      }
    }

    // PHPantom is stricter than Psalm here: since ["a","b","c"] is non-empty,
    // the pre-loop null cannot survive into post-loop scope.
    assertType('int|string', $tag);
}

// Test: bleedVarIntoOuterContextWithRedefineAndBreak
namespace PsalmTest_loop_foreach_11 {
    $tag = null;
    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $tag = null;
      } else {
        $tag = null;
        break;
      }
    }

    assertType('null', $tag);
}

// Test: nullToMixedWithNullCheckNoContinue
namespace PsalmTest_loop_foreach_12 {
    function getStrings(): array {
        return ["hello", "world"];
    }

    $a = null;

    foreach (getStrings() as $s) {
      if ($a === null) {
        $a = $s;
      }
    }

    assertType('mixed', $a);
}

// Test: nullToMixedWithNullCheckAndContinue
namespace PsalmTest_loop_foreach_13 {
    $a = null;

    function getStrings(): array {
        return ["hello", "world"];
    }

    $a = null;

    foreach (getStrings() as $s) {
      if ($a === null) {
        $a = $s;
        continue;
      }
    }

    assertType('mixed', $a);
}

// Test: falseToBoolExplicitBreak
namespace PsalmTest_loop_foreach_14 {
    $a = false;

    foreach (["a", "b", "c"] as $tag) {
      $a = true;
      break;
    }

    assertType('bool', $a);
}

// Test: falseToBoolExplicitContinue
namespace PsalmTest_loop_foreach_15 {
    $a = false;

    foreach (["a", "b", "c"] as $tag) {
      $a = true;
      continue;
    }

    assertType('bool', $a);
}

// Test: falseToBoolInBreak
namespace PsalmTest_loop_foreach_16 {
    $a = false;

    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $a = true;
        break;
      } else {
        $a = true;
        break;
      }
    }

    assertType('bool', $a);
}

// Test: falseToBoolInContinue
namespace PsalmTest_loop_foreach_17 {
    $a = false;

    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $a = true;
        continue;
      }
    }

    assertType('bool', $a);
}

// Test: falseToBoolInBreakAndContinue
namespace PsalmTest_loop_foreach_18 {
    $a = false;

    foreach (["a", "b", "c"] as $tag) {
      if ($tag === "a") {
        $a = true;
        break;
      }

      if ($tag === "b") {
        $a = true;
        continue;
      }
    }

    assertType('bool', $a);
}

// Test: falseToBoolInNestedForeach
namespace PsalmTest_loop_foreach_19 {
    $a = false;

    foreach (["d", "e", "f"] as $l) {
        foreach (["a", "b", "c"] as $tag) {
            if (!$a) {
                if (rand(0, 10)) {
                    $a = true;
                    break;
                } else {
                    $a = true;
                    break;
                }
            }
        }
    }

    assertType('bool', $a);
}

// Test: falseToBoolAfterContinueAndBreak
namespace PsalmTest_loop_foreach_20 {
    $a = false;
    foreach ([1, 2, 3] as $i) {
      if ($i > 1) {
        $a = true;
        continue;
      }

      break;
    }

    assertType('bool', $a);
}


# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B1. `static`/`$this` return types are rejected where a `Stringable` object is accepted

**Impact: Medium · Effort: Low**

`type_mismatch_argument` flags passing a `static(Foo)` or `$this(Foo)`
typed value to a `string` parameter even when `Foo` implements
`Stringable`, which PHP accepts (it calls `__toString()`). Minimal
reproduction:

```php
class Node implements Stringable {
    private function __get($name): static {}
    public function __toString(): string { return ''; }
}
$n = new Node();
throw new \Exception($n->Body->Msg);
// Argument 1 ($message) expects string, got static(Node)
```

The cause is `PhpType::is_object_like` in `src/php_type/mod.rs`: it
answers `true` for `Named`, `Generic`, `ObjectShape` and `Nullable`, but
`false` for `TypeKind::StaticType` and `TypeKind::ThisType`. The
`Stringable` branch in
`src/diagnostics/type_errors/compatibility.rs::is_type_compatible` is
gated on `is_object_like()`, so it never runs for those two kinds and the
check falls through to a mismatch. `base_name()` right next to it *does*
handle both kinds, which is what makes the diagnostic message name the
bound class correctly while the compatibility check ignores it.

Every other consumer of `is_object_like` is affected the same way, so fix
it there rather than in the diagnostic. Real-world hit: any use of
`SimpleXMLElement`, whose stubbed `__get` returns `static`, so
`(string) $xml->Body->Message` style code reports a false positive on
every argument passed to a `string` parameter.

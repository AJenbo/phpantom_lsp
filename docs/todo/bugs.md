# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B2. A layout's contract is enforced without the layout's own suppliers

`blade_template_contract` (`src/blade/contract.rs`) merges the layouts a
template `@extends` into the contract call sites are judged against, so a
variable only `layouts/app.blade.php` declares is one every caller of
`pages.home` has to pass. The `supplied` set that exempts a variable from
the missing check is built from the *child* alone:
`declares_component_directive`, `extract_props`, `extract_aware`,
`blade_backing_class_vars`, and `blade_provider_vars` all run against the
child's source and the child's view names.

A view composer is normally registered on the template that reads the
variable, which is the layout. `View::composer('layouts.app', …)` sharing
`$title` types the layout's `$title` correctly, because
`blade_provider_vars` matches the registered pattern against
`layouts.app`, but nothing matches it against `pages.home`, so every page
extending that layout reports "View 'pages.home' expects $title of type
string, which is not passed" at every one of its call sites. Nothing the
caller can write clears it. The same hole drops a layout's own `@props`
defaults and, for a layout that is itself a component, its backing class
members.

Fix: build `supplied` over the whole chain the way `vars` already is. For
each level the layout walk yields, add that template's props and aware
defaults, its component scope, its backing class members, and its
provider vars. `blade_layout_chain` returns the view name alongside the
source, so the names each level needs are already in hand.
`blade_rendering_scope` has the same shape and needs the same fix.

### B3. A component's render scope over-supplies and hides missing variables

`component_render_scope_names` (`src/blade/backing_class.rs`) answers
"what does the class enclosing this `view()` call put in the rendered
template's scope", and the call-site missing check treats every name it
returns as supplied. It collects every public non-static property and
every public non-static method of the fully resolved class and stops
there. Its sibling `class_member_vars`, which feeds the same members into
the template's own prologue, filters three things it does not: members
declared on the framework base class, `__`-prefixed names, and methods
that require an argument. It also gates methods behind `include_methods`,
false for Livewire, because a Livewire public method is an action rather
than view data.

So a `view()` call inside a Blade component treats `data`, `render`,
`withAttributes`, `shouldRender`, and the rest of
`Illuminate\View\Component`'s public surface as supplied, and one inside a
Livewire component treats every action the component defines as supplied.
A template declaring `@var array $data` is then never reported as missing
it. Bladestan's `ClassPropertiesResolver` reads public properties only
(plus `slot` for a component and the instance variables for Livewire), so
it has no such hole.

Fix: take the names from `class_member_vars`, which already computes
exactly this member set under the right rules, rather than walking the
resolved members a second time under different ones. One member list, one
set of rules, so the prologue and the diagnostic cannot disagree about
what a component hands its view.

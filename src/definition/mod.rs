/// Goto definition support.
///
/// This module contains the logic for resolving "go to definition" requests,
/// allowing users to jump from a symbol reference to its definition in the
/// source code.
///
/// Supported symbols:
///   - **Class-like types**: class, interface, trait, enum references
///   - **Methods**: `$this->method()`, `self::method()`, `MyClass::method()`, `$var->method()`
///   - **Properties**: `$this->property`, `$var->property`, `MyClass::$staticProp`
///   - **Constants**: `self::MY_CONST`, `MyClass::MY_CONST`, `parent::MY_CONST`
///   - **Chained access**: `$this->prop->method()`
///   - **Variables**: `$var` jumps to the most recent assignment or declaration
///     (assignment, parameter, foreach, catch, static/global)
///
/// - [`resolve`]: Core entry points — word extraction, name resolution,
///   same-file / PSR-4 definition lookup, `self`/`static`/`parent` handling,
///   and standalone function definition resolution.
/// - [`member`]: Member-access resolution — `->`, `?->`, `::` operator
///   detection, subject extraction, member classification, inheritance-chain
///   walking (parent classes, traits, mixins), and member position lookup.
/// - [`variable`]: Variable definition resolution — `$var` jump-to-definition,
///   assignment / parameter / foreach / catch detection, and type-hint
///   resolution at definition sites.
mod member;
mod resolve;
mod variable;

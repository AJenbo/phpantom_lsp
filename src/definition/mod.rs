/// Goto definition and go-to-implementation support.
///
/// This module contains the logic for resolving "go to definition" and
/// "go to implementation" requests, allowing users to jump from a symbol
/// reference to its definition or concrete implementations.
///
/// Supported symbols (definition):
///   - **Class-like types**: class, interface, trait, enum references
///   - **Methods**: `$this->method()`, `self::method()`, `MyClass::method()`, `$var->method()`
///   - **Properties**: `$this->property`, `$var->property`, `MyClass::$staticProp`
///   - **Constants**: `self::MY_CONST`, `MyClass::MY_CONST`, `parent::MY_CONST`
///   - **Chained access**: `$this->prop->method()`
///   - **Variables**: `$var` jumps to the most recent assignment or declaration
///     (assignment, parameter, foreach, catch, static/global)
///
/// Supported symbols (implementation):
///   - **Interface names**: jumps to all classes that implement the interface
///   - **Abstract class names**: jumps to all classes that extend the abstract class
///   - **Method calls on interfaces/abstract classes**: jumps to the concrete
///     method implementations in all implementing/extending classes
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
/// - [`implementation`]: Go-to-implementation — finding concrete classes that
///   implement an interface or extend an abstract class, and locating the
///   concrete method definitions within those classes.
mod implementation;
pub(crate) mod member;
mod resolve;
mod variable;

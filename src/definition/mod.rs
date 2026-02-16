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
/// - [`resolve`]: Word extraction, member access detection, name resolution,
///   variable definition lookup, and definition location lookup.
mod resolve;

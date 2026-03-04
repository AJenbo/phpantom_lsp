//! Data types used throughout the PHPantom server.
//!
//! This module contains all the "model" structs and enums that represent
//! extracted PHP information (classes, methods, properties, constants,
//! standalone functions) as well as completion-related types
//! (AccessKind, CompletionTarget, SubjectExpr), PHPStan conditional
//! return type representations, PHPStan/Psalm array shape types, and
//! the [`PhpVersion`] type used for version-aware stub filtering.

// Re-export SubjectExpr and BracketSegment from their canonical module
// so that existing `use crate::types::{SubjectExpr, BracketSegment, …}`
// paths continue to work.
pub use crate::subject_expr::{BracketSegment, SubjectExpr};

use std::collections::HashMap;
use std::fmt;

// ─── PHP Version ────────────────────────────────────────────────────────────

/// A PHP major.minor version used for version-aware stub filtering.
///
/// phpstorm-stubs annotate functions, methods, and parameters with
/// `#[PhpStormStubsElementAvailable(from: 'X.Y', to: 'X.Y')]` attributes
/// to indicate which PHP versions they apply to.  PHPantom uses this
/// struct to decide which variant of a stub element to present.
///
/// The version is detected from `composer.json` (`require.php`) during
/// server initialization. When no version is found, [`PhpVersion::default`]
/// returns PHP 8.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhpVersion {
    /// Major version number (e.g. `8` in PHP 8.4).
    pub major: u8,
    /// Minor version number (e.g. `4` in PHP 8.4).
    pub minor: u8,
}

impl PhpVersion {
    /// Create a new `PhpVersion`.
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// Parse a PHP version from a Composer `require.php` constraint string.
    ///
    /// Extracts the first `major.minor` pair found in the constraint.
    /// Supports common formats:
    ///   - `"^8.4"` → 8.4
    ///   - `">=8.3"` → 8.3
    ///   - `"~8.2"` → 8.2
    ///   - `"8.1.*"` → 8.1
    ///   - `">=8.0 <8.4"` → 8.0 (first match wins)
    ///   - `"8.3.1"` → 8.3
    ///   - `"^8"` → 8.0
    ///
    /// Returns `None` if no version can be extracted.
    pub fn from_composer_constraint(constraint: &str) -> Option<Self> {
        // Walk through the constraint looking for digit sequences that
        // form a major.minor version.  Skip common prefix operators.
        let s = constraint.trim();

        // Try each whitespace/pipe-separated segment, return the first match.
        for segment in s.split(['|', ' ']) {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }

            // Strip leading operator characters: ^, ~, >=, <=, >, <, =, !
            let digits_start = segment
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or(segment.len());
            let version_part = &segment[digits_start..];

            if version_part.is_empty() {
                continue;
            }

            let mut parts = version_part.split('.');
            if let Some(major_str) = parts.next()
                && let Ok(major) = major_str.parse::<u8>()
            {
                let minor = parts
                    .next()
                    .and_then(|s| s.trim_end_matches('*').parse::<u8>().ok())
                    .unwrap_or(0);
                return Some(Self { major, minor });
            }
        }

        None
    }

    /// Returns `true` if the given `from`..`to` version range includes
    /// this PHP version.
    ///
    /// - `from` is inclusive: the element is available starting at that version.
    /// - `to` is inclusive: the element is available up to and including that version.
    /// - When `from` is `None`, there is no lower bound.
    /// - When `to` is `None`, there is no upper bound.
    pub fn matches_range(&self, from: Option<PhpVersion>, to: Option<PhpVersion>) -> bool {
        if let Some(lower) = from
            && (self.major, self.minor) < (lower.major, lower.minor)
        {
            return false;
        }
        if let Some(upper) = to
            && (self.major, self.minor) > (upper.major, upper.minor)
        {
            return false;
        }
        true
    }
}

impl Default for PhpVersion {
    /// Default PHP version when none is detected: 8.5.
    fn default() -> Self {
        Self { major: 8, minor: 5 }
    }
}

impl fmt::Display for PhpVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// The return type of `Backend::extract_class_like_members`.
///
/// Contains `(methods, properties, constants, used_traits, trait_precedences, trait_aliases)`
/// extracted from the members of a class-like declaration.
/// Extracted class-like members from a class body.
///
/// Fields: methods, properties, constants, used_traits, trait_precedences,
/// trait_aliases, inline_use_generics.
///
/// The last element holds `@use` generics extracted from docblocks on trait
/// `use` statements inside the class body (e.g. `/** @use BuildsQueries<TModel> */`).
pub type ExtractedMembers = (
    Vec<MethodInfo>,
    Vec<PropertyInfo>,
    Vec<ConstantInfo>,
    Vec<String>,
    Vec<TraitPrecedence>,
    Vec<TraitAlias>,
    Vec<(String, Vec<String>)>,
);

// ─── Array Shape Types ──────────────────────────────────────────────────────

/// A single entry in a PHPStan/Psalm array shape type.
///
/// Array shapes describe the exact structure of an array, including
/// named or positional keys and their value types.
///
/// # Examples
///
/// ```text
/// array{name: string, age: int}       → two entries with keys "name" and "age"
/// array{0: User, 1: Address}          → two entries with numeric keys
/// array{name: string, age?: int}      → "age" is optional
/// array{string, int}                  → implicit keys "0" and "1"
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayShapeEntry {
    /// The key name (e.g. `"name"`, `"0"`, `"1"`).
    /// For positional entries without explicit keys, this is the
    /// stringified index (`"0"`, `"1"`, …).
    pub key: String,
    /// The value type string (e.g. `"string"`, `"int"`, `"User"`).
    pub value_type: String,
    /// Whether this key is optional (declared with `?` suffix, e.g. `age?: int`).
    pub optional: bool,
}

/// Variance of a `@template` parameter.
///
/// PHPStan and Psalm support `@template-covariant` and
/// `@template-contravariant` to express variance constraints on generic
/// type parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemplateVariance {
    /// No variance annotation (`@template T`).
    #[default]
    Invariant,
    /// `@template-covariant T`
    Covariant,
    /// `@template-contravariant T`
    Contravariant,
}

impl TemplateVariance {
    /// Returns the tag name used in PHPDoc for this variance.
    pub fn tag_name(self) -> &'static str {
        match self {
            Self::Invariant => "template",
            Self::Covariant => "template-covariant",
            Self::Contravariant => "template-contravariant",
        }
    }
}

/// Visibility of a class member (method, property, or constant).
///
/// In PHP, members without an explicit visibility modifier default to `Public`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Protected,
    Private,
}

/// Stores extracted parameter information from a parsed PHP method.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// The parameter name including the `$` prefix (e.g. "$text").
    pub name: String,
    /// Whether this parameter is required (no default value and not variadic).
    pub is_required: bool,
    /// Effective type hint string after docblock override (e.g. "list<User>").
    ///
    /// When a `@param` tag is present in the docblock and is more specific
    /// than the native PHP type hint, this holds the docblock type.
    /// Otherwise it holds the native type hint unchanged.
    pub type_hint: Option<String>,
    /// The native PHP type hint as written in source code (e.g. "array", "string").
    ///
    /// Preserved separately so that hover can show the actual PHP declaration
    /// in the code block while displaying the richer docblock type alongside
    /// the FQN header.  `None` when no type hint is present in source.
    pub native_type_hint: Option<String>,
    /// Human-readable description extracted from the `@param` tag.
    ///
    /// For `@param list<User> $users The active users`, this would be
    /// `Some("The active users")`.  `None` when no description text
    /// follows the parameter name in the `@param` tag.
    pub description: Option<String>,
    /// The source text of the default value expression (e.g. `"0"`, `"null"`,
    /// `"[]"`, `"'hello'"`).
    ///
    /// Extracted from the AST span when the parameter has a default value.
    /// `None` when the parameter has no default.
    pub default_value: Option<String>,
    /// Whether this parameter is variadic (has `...`).
    pub is_variadic: bool,
    /// Whether this parameter is passed by reference (has `&`).
    pub is_reference: bool,
}

/// Stores extracted method information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// The method name (e.g. "updateText").
    pub name: String,
    /// Byte offset of the method's name token in the source file.
    ///
    /// Set to the `span.start.offset` of the name `LocalIdentifier` during
    /// parsing.  A value of `0` means "not available" (e.g. for stubs and
    /// synthetic members) — callers should fall back to text search.
    pub name_offset: u32,
    /// The parameters of the method.
    pub parameters: Vec<ParameterInfo>,
    /// Effective return type hint after docblock override (e.g. "Collection<User>").
    ///
    /// When a `@return` tag is present in the docblock and is more specific
    /// than the native PHP return type hint, this holds the docblock type.
    /// Otherwise it holds the native type hint unchanged.
    pub return_type: Option<String>,
    /// The native PHP return type hint as written in source code (e.g. "array", "self").
    ///
    /// Preserved separately so that hover can show the actual PHP declaration
    /// in the code block while displaying the richer docblock type alongside
    /// the FQN header.  `None` when no return type hint is present in source.
    pub native_return_type: Option<String>,
    /// Human-readable description extracted from the method's docblock.
    ///
    /// This is the free-text portion of the docblock (before any `@tag` lines).
    /// `None` when the docblock has no description or no docblock is present.
    pub description: Option<String>,
    /// Human-readable description extracted from the `@return` tag.
    ///
    /// For `@return list<User> The active users`, this would be
    /// `Some("The active users")`.  `None` when no description text
    /// follows the type in the `@return` tag.
    pub return_description: Option<String>,
    /// URL from the `@link` tag in the docblock.
    ///
    /// For `@link https://php.net/manual/en/function.array-map.php`,
    /// this would be `Some("https://php.net/manual/en/function.array-map.php")`.
    /// `None` when no `@link` tag is present.
    pub link: Option<String>,
    /// Whether the method is static.
    pub is_static: bool,
    /// Visibility of the method (public, protected, or private).
    pub visibility: Visibility,
    /// Optional PHPStan conditional return type parsed from the docblock.
    ///
    /// When present, the resolver should use this instead of `return_type`
    /// and resolve the concrete type based on call-site arguments.
    ///
    /// Example docblock:
    /// ```text
    /// @return ($abstract is class-string<TClass> ? TClass : mixed)
    /// ```
    pub conditional_return: Option<ConditionalReturnType>,
    /// Deprecation message from the `@deprecated` PHPDoc tag.
    ///
    /// `None` means not deprecated. `Some("")` means deprecated without a
    /// message. `Some("Use foo() instead")` includes the explanation.
    pub deprecation_message: Option<String>,
    /// Template parameter names declared via `@template` tags in the
    /// method-level docblock.
    ///
    /// For example, a method with `@template T of Model` would have
    /// `template_params: vec!["T".into()]`.
    ///
    /// These are distinct from class-level template parameters
    /// (`ClassInfo::template_params`) and are used for general
    /// method-level generic type substitution at call sites.
    pub template_params: Vec<String>,
    /// Mappings from method-level template parameter names to the method
    /// parameter names (with `$` prefix) that directly bind them via
    /// `@param` annotations.
    ///
    /// For example, `@template T` + `@param T $model` produces
    /// `[("T", "$model")]`.  At call sites the resolver uses these
    /// bindings to infer concrete types for each template parameter
    /// from the actual argument expressions.
    pub template_bindings: Vec<(String, String)>,
    /// Whether this method has the `#[Scope]` attribute (Laravel 11+).
    ///
    /// Methods decorated with `#[\Illuminate\Database\Eloquent\Attributes\Scope]`
    /// are treated as Eloquent scope methods without needing the `scopeX`
    /// naming convention.  The method's own name is used directly as the
    /// public-facing scope name (e.g. `#[Scope] protected function active()`
    /// becomes `User::active()`).
    pub has_scope_attribute: bool,
    /// Whether this method is a virtual (synthesized) member.
    ///
    /// Virtual methods come from `@method` docblock tags, `@mixin` classes,
    /// or framework-specific providers (e.g. Laravel model scopes).  They
    /// have no real declaration in source code.
    ///
    /// Set to `true` by [`MethodInfo::virtual_method`] and by providers;
    /// set to `false` by the parser for real declared methods.
    pub is_virtual: bool,
}

impl MethodInfo {
    /// Create a virtual `MethodInfo` with sensible defaults.
    ///
    /// The method is public, non-static, non-deprecated, with no
    /// parameters, no template params, and `name_offset: 0`.
    ///
    /// Use struct update syntax to override individual fields:
    ///
    /// ```ignore
    /// MethodInfo {
    ///     is_static: true,
    ///     parameters: params,
    ///     ..MethodInfo::virtual_method("foo", Some("string"))
    /// }
    /// ```
    pub fn virtual_method(name: &str, return_type: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            name_offset: 0,
            parameters: Vec::new(),
            return_type: return_type.map(|s| s.to_string()),
            native_return_type: None,
            description: None,
            return_description: None,
            link: None,
            is_static: false,
            visibility: Visibility::Public,
            conditional_return: None,
            deprecation_message: None,
            template_params: Vec::new(),
            template_bindings: Vec::new(),
            has_scope_attribute: false,
            is_virtual: true,
        }
    }
}

/// Stores extracted property information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// The property name WITHOUT the `$` prefix (e.g. "name", "age").
    /// This matches PHP access syntax: `$this->name` not `$this->$name`.
    pub name: String,
    /// Byte offset of the property's variable token (`$name`) in the source file.
    ///
    /// Set to the `span.start.offset` of the `DirectVariable` during parsing.
    /// A value of `0` means "not available" — callers should fall back to
    /// text search.
    pub name_offset: u32,
    /// Effective type hint string after docblock override (e.g. "list<User>").
    ///
    /// When a `@var` tag is present in the docblock and is more specific
    /// than the native PHP type hint, this holds the docblock type.
    /// Otherwise it holds the native type hint unchanged.
    pub type_hint: Option<String>,
    /// The native PHP type hint as written in source code (e.g. "array", "string").
    ///
    /// Preserved separately so that hover can show the actual PHP declaration
    /// in the code block while displaying the richer docblock type alongside
    /// the FQN header.  `None` when no type hint is present in source.
    pub native_type_hint: Option<String>,
    /// Human-readable description extracted from the property's docblock.
    ///
    /// This is the free-text portion of the docblock (before any `@tag` lines).
    /// `None` when the docblock has no description or no docblock is present.
    pub description: Option<String>,
    /// Whether the property is static.
    pub is_static: bool,
    /// Visibility of the property (public, protected, or private).
    pub visibility: Visibility,
    /// Deprecation message from the `@deprecated` PHPDoc tag.
    ///
    /// `None` means not deprecated. `Some("")` means deprecated without a
    /// message. `Some("Use foo() instead")` includes the explanation.
    pub deprecation_message: Option<String>,
    /// Whether this property is a virtual (synthesized) member.
    ///
    /// Virtual properties come from `@property` / `@property-read` /
    /// `@property-write` docblock tags, `@mixin` classes, or
    /// framework-specific providers (e.g. Laravel model columns).
    /// They have no real declaration in source code.
    ///
    /// Set to `true` by [`PropertyInfo::virtual_property`] and by
    /// providers; set to `false` by the parser for real declared
    /// properties.
    pub is_virtual: bool,
}

impl PropertyInfo {
    /// Create a virtual `PropertyInfo` with sensible defaults.
    ///
    /// The property is public, non-static, with no deprecation message and
    /// `name_offset: 0`.
    ///
    /// Use struct update syntax to override individual fields:
    ///
    /// ```ignore
    /// PropertyInfo {
    ///     deprecation_message: Some("Use newProp instead".into()),
    ///     ..PropertyInfo::virtual_property("foo", Some("string"))
    /// }
    /// ```
    pub fn virtual_property(name: &str, type_hint: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            name_offset: 0,
            type_hint: type_hint.map(|s| s.to_string()),
            native_type_hint: None,
            description: None,
            is_static: false,
            visibility: Visibility::Public,
            deprecation_message: None,
            is_virtual: true,
        }
    }
}

/// Stores extracted constant information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct ConstantInfo {
    /// The constant name (e.g. "MAX_SIZE", "STATUS_ACTIVE").
    pub name: String,
    /// Byte offset of the constant's name token in the source file.
    ///
    /// Set to the `span.start.offset` of the name `LocalIdentifier` during
    /// parsing.  A value of `0` means "not available" — callers should fall
    /// back to text search.
    pub name_offset: u32,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
    /// Visibility of the constant (public, protected, or private).
    pub visibility: Visibility,
    /// Deprecation message from the `@deprecated` PHPDoc tag.
    ///
    /// `None` means not deprecated. `Some("")` means deprecated without a
    /// message. `Some("Use OK instead")` includes the explanation.
    pub deprecation_message: Option<String>,
    /// Human-readable description extracted from the constant's docblock.
    ///
    /// This is the free-text portion of the docblock (before any `@tag` lines).
    /// `None` when the docblock has no description or no docblock is present.
    pub description: Option<String>,
    /// Whether this constant is an enum case rather than a regular class constant.
    pub is_enum_case: bool,
    /// The literal value of a backed enum case (e.g. `"'pending'"` for
    /// `case Pending = 'pending';`).  `None` for unit enum cases and
    /// regular class constants.
    pub enum_value: Option<String>,
    /// The initializer expression source text for a regular class constant
    /// (e.g. `"'active'"` for `const STATUS = 'active';`, `"100"` for
    /// `const LIMIT = 100;`).  `None` when the constant has no initializer
    /// or the source text could not be extracted.
    pub value: Option<String>,
    /// Whether this constant is a virtual (synthesized) member.
    ///
    /// Virtual constants come from `@mixin` classes or framework-specific
    /// providers.  They have no real declaration in source code.
    ///
    /// Set to `true` by providers; set to `false` by the parser for real
    /// declared constants.
    pub is_virtual: bool,
}

/// Describes the access operator that triggered completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Completion triggered after `->` (instance access).
    Arrow,
    /// Completion triggered after `::` (static access).
    DoubleColon,
    /// Completion triggered after `parent::`, `self::`, or `static::`.
    ///
    /// All three keywords use `::` syntax but differ from external static
    /// access (`ClassName::`): they show both static **and** instance
    /// methods (PHP allows `self::nonStaticMethod()`,
    /// `static::nonStaticMethod()`, and `parent::nonStaticMethod()` from
    /// an instance context), plus constants and static properties.
    /// Visibility filtering (e.g. excluding private members for `parent::`)
    /// is handled separately via `current_class_name`.
    ParentDoubleColon,
    /// No specific access operator detected (e.g. inside class body).
    Other,
}

/// The result of analysing what is to the left of `->` or `::`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionTarget {
    /// Whether `->` or `::` was used.
    pub access_kind: AccessKind,
    /// The textual subject before the operator, e.g. `"$this"`, `"self"`,
    /// `"$var"`, `"$this->prop"`, `"ClassName"`.
    pub subject: String,
}

// ─── Resolved Callable Target ───────────────────────────────────────────────

/// The result of resolving a call expression to its callable target.
///
/// Shared between signature help (`resolve_callable`) and named-argument
/// completion (`resolve_named_arg_params`).  Each caller projects the
/// fields it needs from the result.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedCallableTarget {
    /// The parameters of the callable.
    pub parameters: Vec<ParameterInfo>,
    /// Optional return type string.
    pub return_type: Option<String>,
}
/// Stores extracted information about a standalone PHP function.
///
/// This is used for global / namespaced functions defined outside of classes,
/// typically found in files listed by Composer's `autoload_files.php`.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// The function name (e.g. "array_map", "myHelper").
    pub name: String,
    /// Byte offset of the function's name token in the source file.
    ///
    /// Set to the `span.start.offset` of the name `LocalIdentifier` during
    /// parsing.  A value of `0` means "not available" (e.g. for stubs and
    /// synthetic entries) — callers should fall back to text search.
    pub name_offset: u32,
    /// The parameters of the function.
    pub parameters: Vec<ParameterInfo>,
    /// Effective return type hint after docblock override (e.g. "Collection<User>").
    ///
    /// When a `@return` tag is present in the docblock and is more specific
    /// than the native PHP return type hint, this holds the docblock type.
    /// Otherwise it holds the native type hint unchanged.
    pub return_type: Option<String>,
    /// The native PHP return type hint as written in source code (e.g. "array", "self").
    ///
    /// Preserved separately so that hover can show the actual PHP declaration
    /// in the code block while displaying the richer docblock type alongside
    /// the FQN header.  `None` when no return type hint is present in source.
    pub native_return_type: Option<String>,
    /// Human-readable description extracted from the function's docblock.
    ///
    /// This is the free-text portion of the docblock (before any `@tag` lines).
    /// `None` when the docblock has no description or no docblock is present.
    pub description: Option<String>,
    /// Human-readable description extracted from the `@return` tag.
    ///
    /// For `@return list<User> The active users`, this would be
    /// `Some("The active users")`.  `None` when no description text
    /// follows the type in the `@return` tag.
    pub return_description: Option<String>,
    /// URL from the `@link` tag in the docblock.
    ///
    /// For `@link https://php.net/manual/en/function.array-map.php`,
    /// this would be `Some("https://php.net/manual/en/function.array-map.php")`.
    /// `None` when no `@link` tag is present.
    pub link: Option<String>,
    /// The namespace this function is declared in, if any.
    /// For example, `Amp\delay` would have namespace `Some("Amp")`.
    pub namespace: Option<String>,
    /// Optional PHPStan conditional return type parsed from the docblock.
    ///
    /// When present, the resolver should use this instead of `return_type`
    /// and resolve the concrete type based on call-site arguments.
    ///
    /// Example docblock:
    /// ```text
    /// @return ($abstract is class-string<TClass> ? TClass : \Illuminate\Foundation\Application)
    /// ```
    pub conditional_return: Option<ConditionalReturnType>,
    /// Type assertions parsed from `@phpstan-assert` / `@psalm-assert`
    /// annotations in the function's docblock.
    ///
    /// These allow user-defined functions to act as custom type guards,
    /// narrowing the type of a parameter after the call (or conditionally
    /// when used in an `if` condition).
    ///
    /// Example docblocks:
    /// ```text
    /// @phpstan-assert User $value           — unconditional assertion
    /// @phpstan-assert !User $value          — negated assertion
    /// @phpstan-assert-if-true User $value   — assertion when return is true
    /// @phpstan-assert-if-false User $value  — assertion when return is false
    /// ```
    pub type_assertions: Vec<TypeAssertion>,
    /// Deprecation message from the `@deprecated` PHPDoc tag.
    ///
    /// `None` means not deprecated. `Some("")` means deprecated without a
    /// message. `Some("Use newHelper() instead")` includes the explanation.
    pub deprecation_message: Option<String>,
}

// ─── PHPStan Type Assertions ────────────────────────────────────────────────

/// A type assertion annotation parsed from `@phpstan-assert` /
/// `@psalm-assert` (and their `-if-true` / `-if-false` variants).
///
/// These annotations let any function or method act as a custom type
/// guard, telling the analyser that a parameter has been narrowed to
/// a specific type after the call succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAssertion {
    /// When the assertion applies.
    pub kind: AssertionKind,
    /// The parameter name **with** the `$` prefix (e.g. `"$value"`).
    pub param_name: String,
    /// The asserted type (e.g. `"User"`, `"AdminUser"`).
    pub asserted_type: String,
    /// Whether the assertion is negated (`!Type`), meaning the parameter
    /// is guaranteed to *not* be this type.
    pub negated: bool,
}

/// When a `@phpstan-assert` annotation takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssertionKind {
    /// `@phpstan-assert` — unconditional: after the function returns
    /// (without throwing), the assertion holds for all subsequent code.
    Always,
    /// `@phpstan-assert-if-true` — the assertion holds when the function
    /// returns `true` (i.e. inside the `if` body).
    IfTrue,
    /// `@phpstan-assert-if-false` — the assertion holds when the function
    /// returns `false` (i.e. inside the `else` body, or the `if` body of
    /// a negated condition).
    IfFalse,
}

// ─── PHPStan Conditional Return Types ───────────────────────────────────────

/// A parsed PHPStan conditional return type expression.
///
/// PHPStan allows `@return` annotations that conditionally resolve to
/// different types based on the value/type of a parameter.  For example:
///
/// ```text
/// @return ($abstract is class-string<TClass> ? TClass
///           : ($abstract is null ? \Illuminate\Foundation\Application : mixed))
/// ```
///
/// This enum represents the recursive structure of such expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionalReturnType {
    /// A concrete (terminal) type, e.g. `\Illuminate\Foundation\Application`
    /// or `mixed`.
    Concrete(String),

    /// A conditional branch:
    /// `($param is Condition ? ThenType : ElseType)`
    Conditional {
        /// The parameter name **without** the `$` prefix (e.g. `"abstract"`).
        param_name: String,
        /// The condition being checked.
        condition: ParamCondition,
        /// The type when the condition is satisfied.
        then_type: Box<ConditionalReturnType>,
        /// The type when the condition is not satisfied.
        else_type: Box<ConditionalReturnType>,
    },
}

/// The kind of condition in a PHPStan conditional return type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamCondition {
    /// `$param is class-string<T>` — when the argument is a `::class` constant,
    /// the return type is the class itself.
    ClassString,

    /// `$param is null` — typically used for parameters with `= null` defaults
    /// to return a known concrete type when no argument is provided.
    IsNull,

    /// `$param is \SomeType` — a general type check (e.g. `\Closure`, `string`).
    IsType(String),
}

/// A trait `insteadof` adaptation.
///
/// When a class uses multiple traits that define the same method, PHP
/// requires an explicit `insteadof` declaration to resolve the conflict.
///
/// # Example
///
/// ```php
/// use TraitA, TraitB {
///     TraitA::method insteadof TraitB;
/// }
/// ```
///
/// This means TraitA's version of `method` wins and TraitB's is excluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitPrecedence {
    /// The trait that provides the winning method (e.g. `"TraitA"`).
    pub trait_name: String,
    /// The method name being resolved (e.g. `"method"`).
    pub method_name: String,
    /// The traits whose versions of the method are excluded
    /// (e.g. `["TraitB"]`).
    pub insteadof: Vec<String>,
}

/// A trait `as` alias adaptation.
///
/// Creates an alias for a trait method, optionally changing its visibility.
///
/// # Examples
///
/// ```php
/// use TraitA, TraitB {
///     TraitB::method as traitBMethod;          // rename
///     TraitA::method as protected;             // visibility-only change
///     TraitB::method as private altMethod;     // rename + visibility change
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitAlias {
    /// The trait that provides the method (e.g. `Some("TraitB")`).
    /// `None` when the method reference is unqualified (e.g. `method as …`).
    pub trait_name: Option<String>,
    /// The original method name (e.g. `"method"`).
    pub method_name: String,
    /// The alias name, if any (e.g. `Some("traitBMethod")`).
    /// `None` when only the visibility is changed (e.g. `method as protected`).
    pub alias: Option<String>,
    /// Optional visibility override (e.g. `Some(Visibility::Protected)`).
    pub visibility: Option<Visibility>,
}

/// The syntactic kind of a class-like declaration.
///
/// PHP has four class-like constructs that share the same `ClassInfo`
/// representation.  This enum lets callers distinguish them when the
/// difference matters (e.g. `throw new` completion should only offer
/// concrete classes, not interfaces or traits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClassLikeKind {
    /// A regular `class` declaration (the default).
    #[default]
    Class,
    /// An `interface` declaration.
    Interface,
    /// A `trait` declaration.
    Trait,
    /// An `enum` declaration.
    Enum,
}

/// Laravel-specific metadata extracted from Eloquent model classes.
///
/// Grouped into a sub-struct to keep the core `ClassInfo` focused on
/// PHP semantics. All fields default to empty/`None`, so non-Laravel
/// classes carry no overhead beyond a single struct value.
#[derive(Debug, Clone, Default)]
pub struct LaravelMetadata {
    /// Custom collection class for Eloquent models.
    ///
    /// Detected from three Laravel mechanisms:
    ///
    /// 1. The `#[CollectedBy(CustomCollection::class)]` attribute on the
    ///    model class.
    /// 2. The `/** @use HasCollection<CustomCollection> */` docblock
    ///    annotation on a `use HasCollection;` trait usage.
    /// 3. A `newCollection()` method override returning a custom type.
    ///
    /// When set, the `LaravelModelProvider` replaces
    /// `\Illuminate\Database\Eloquent\Collection` with this class in
    /// relationship property types and Builder-forwarded return types
    /// (e.g. `get()`, `all()`).
    pub custom_collection: Option<String>,
    /// Eloquent cast definitions extracted from the `$casts` property
    /// initializer or the `casts()` method body.
    ///
    /// Each entry maps a column name to a cast type string (e.g.
    /// `("created_at", "datetime")`, `("is_admin", "boolean")`).
    /// The `LaravelModelProvider` uses these to synthesize typed virtual
    /// properties, mapping cast type strings to PHP types (e.g.
    /// `datetime` to `Carbon\Carbon`, `boolean` to `bool`).
    pub casts_definitions: Vec<(String, String)>,
    /// Eloquent attribute defaults extracted from the `$attributes`
    /// property initializer.
    ///
    /// Each entry maps a column name to a PHP type string inferred from
    /// the literal default value (e.g. `("role", "string")`,
    /// `("is_active", "bool")`, `("login_count", "int")`).
    /// The `LaravelModelProvider` uses these as a fallback when no
    /// `$casts` entry exists for the same column.
    pub attributes_definitions: Vec<(String, String)>,
    /// Column names extracted from `$fillable`, `$guarded`, and
    /// `$hidden` property arrays.
    ///
    /// These are simple string lists (no type information), so the
    /// `LaravelModelProvider` synthesizes `mixed`-typed virtual
    /// properties as a last-resort fallback when a column is not
    /// already covered by `$casts` or `$attributes`.
    pub column_names: Vec<String>,
}

/// Stores extracted class information from a parsed PHP file.
/// All data is owned so we don't depend on the parser's arena lifetime.
#[derive(Debug, Clone, Default)]
pub struct ClassInfo {
    /// The syntactic kind of this class-like declaration.
    pub kind: ClassLikeKind,
    /// The name of the class (e.g. "User").
    pub name: String,
    /// The methods defined directly in this class.
    pub methods: Vec<MethodInfo>,
    /// The properties defined directly in this class.
    pub properties: Vec<PropertyInfo>,
    /// The constants defined directly in this class.
    pub constants: Vec<ConstantInfo>,
    /// Byte offset where the class body starts (left brace).
    pub start_offset: u32,
    /// Byte offset where the class body ends (right brace).
    pub end_offset: u32,
    /// Byte offset of the `class` / `interface` / `trait` / `enum` keyword
    /// token in the source file.
    ///
    /// Used with `offset_to_position` to convert directly to an LSP
    /// `Position`.  A value of `0` means "not available" (e.g. for
    /// synthetic classes or anonymous classes) — callers return `None`.
    pub keyword_offset: u32,
    /// The parent class name from the `extends` clause, if any.
    /// This is the raw name as written in source (e.g. "BaseClass", "Foo\\Bar").
    pub parent_class: Option<String>,
    /// Interface names from the `implements` clause (classes and enums only).
    ///
    /// These are resolved to fully-qualified names during post-processing
    /// (see `resolve_parent_class_names` in `parser/ast_update.rs`).
    /// Used by "Go to Implementation" to find classes that implement a
    /// given interface.
    pub interfaces: Vec<String>,
    /// Trait names used by this class via `use TraitName;` statements.
    /// These are resolved to fully-qualified names during post-processing.
    pub used_traits: Vec<String>,
    /// Class names from `@mixin` docblock tags.
    /// These declare that this class exposes public members from the listed
    /// classes via magic methods (`__call`, `__get`, `__set`, etc.).
    /// Resolved to fully-qualified names during post-processing.
    pub mixins: Vec<String>,
    /// Whether the class is declared `final`.
    ///
    /// Final classes cannot be extended, so `static::` is equivalent to
    /// `self::` and need not be offered as a separate completion subject.
    pub is_final: bool,
    /// Whether the class is declared `abstract`.
    ///
    /// Abstract classes cannot be instantiated directly, so they should
    /// be excluded from contexts like `throw new` or `new` completion
    /// where only concrete classes are valid.
    pub is_abstract: bool,
    /// Deprecation message from the `@deprecated` PHPDoc tag.
    ///
    /// `None` means not deprecated. `Some("")` means deprecated without a
    /// message. `Some("Use NewApi instead")` includes the explanation.
    pub deprecation_message: Option<String>,
    /// URL from the `@link` tag in the class-level docblock.
    ///
    /// For `@link https://php.net/manual/en/reserved.classes.php`,
    /// this would be `Some("https://php.net/manual/en/reserved.classes.php")`.
    /// `None` when no `@link` tag is present.
    pub link: Option<String>,
    /// Template parameter names declared via `@template` / `@template-covariant`
    /// / `@template-contravariant` tags in the class-level docblock.
    ///
    /// For example, `Collection` with `@template TKey` and `@template TValue`
    /// would have `template_params: vec!["TKey".into(), "TValue".into()]`.
    pub template_params: Vec<String>,
    /// Upper bounds for template parameters, keyed by parameter name.
    ///
    /// Populated from the `of` clause in `@template` tags. For example,
    /// `@template TNode of PDependNode` produces `("TNode", "PDependNode")`.
    ///
    /// When a type hint resolves to a template parameter name that cannot be
    /// concretely substituted, the resolver falls back to this bound so that
    /// completion and go-to-definition still work against the bound type.
    pub template_param_bounds: HashMap<String, String>,
    /// Generic type arguments from `@extends` / `@phpstan-extends` tags.
    ///
    /// Each entry is `(ClassName, [TypeArg1, TypeArg2, …])`.
    /// For example, `@extends Collection<int, Language>` produces
    /// `("Collection", ["int", "Language"])`.
    pub extends_generics: Vec<(String, Vec<String>)>,
    /// Generic type arguments from `@implements` / `@phpstan-implements` tags.
    ///
    /// Each entry is `(InterfaceName, [TypeArg1, TypeArg2, …])`.
    /// For example, `@implements ArrayAccess<int, User>` produces
    /// `("ArrayAccess", ["int", "User"])`.
    pub implements_generics: Vec<(String, Vec<String>)>,
    /// Generic type arguments from `@use` / `@phpstan-use` tags.
    ///
    /// Each entry is `(TraitName, [TypeArg1, TypeArg2, …])`.
    /// For example, `@use HasFactory<UserFactory>` produces
    /// `("HasFactory", ["UserFactory"])`.
    ///
    /// When a trait declares `@template T` and a class uses it with
    /// `@use SomeTrait<ConcreteType>`, the trait's template parameter `T`
    /// is substituted with `ConcreteType` in all inherited methods and
    /// properties.
    pub use_generics: Vec<(String, Vec<String>)>,
    /// Type aliases defined via `@phpstan-type` / `@psalm-type` tags in the
    /// class-level docblock, and imported via `@phpstan-import-type` /
    /// `@psalm-import-type`.
    ///
    /// Maps alias name → type definition string.
    /// For example, `@phpstan-type UserData array{name: string, email: string}`
    /// produces `("UserData", "array{name: string, email: string}")`.
    ///
    /// These are consulted during type resolution so that a method returning
    /// `UserData` resolves to the underlying `array{name: string, email: string}`.
    pub type_aliases: HashMap<String, String>,
    /// Trait `insteadof` precedence adaptations.
    ///
    /// When a class uses multiple traits with conflicting method names,
    /// `insteadof` declarations specify which trait's version wins.
    /// For example, `TraitA::method insteadof TraitB` means TraitA's
    /// `method` is used and TraitB's is excluded.
    pub trait_precedences: Vec<TraitPrecedence>,
    /// Trait `as` alias adaptations.
    ///
    /// Creates aliases for trait methods, optionally with visibility changes.
    /// For example, `TraitB::method as traitBMethod` adds a new method
    /// `traitBMethod` that is a copy of TraitB's `method`.
    pub trait_aliases: Vec<TraitAlias>,
    /// Raw class-level docblock text, preserved for deferred parsing.
    ///
    /// `@method` and `@property` / `@property-read` / `@property-write`
    /// tags are **not** parsed eagerly into `methods` / `properties`.
    /// Instead, the raw docblock string is stored here and parsed lazily
    /// by the `PHPDocProvider` virtual member provider when completion or
    /// go-to-definition actually needs virtual members.
    ///
    /// Other docblock tags (`@template`, `@extends`, `@deprecated`, etc.)
    /// are still parsed eagerly because they affect class metadata that is
    /// needed during indexing and inheritance resolution.
    pub class_docblock: Option<String>,
    /// The namespace this class was declared in.
    ///
    /// Populated during parsing from the enclosing `namespace { }` block.
    /// For files with a single namespace (the common PSR-4 case) this
    /// matches the file-level namespace.  For files with multiple
    /// namespace blocks (e.g. `example.php` with inline stubs) each class
    /// carries its own namespace so that `find_class_in_ast_map` can
    /// distinguish two classes with the same short name in different
    /// namespace blocks (e.g. `Illuminate\Database\Eloquent\Builder` vs
    /// `Illuminate\Database\Query\Builder`).
    pub file_namespace: Option<String>,
    /// The backing type of a backed enum (e.g. `"string"` or `"int"`).
    /// `None` for unit enums and non-enum class-like declarations.
    pub backed_type: Option<String>,
    /// Laravel-specific metadata (custom collections, casts, attribute
    /// defaults, column names). `None` for non-Laravel classes to avoid
    /// per-class allocation overhead.
    pub laravel: Option<Box<LaravelMetadata>>,
}

// ─── ClassInfo helpers ──────────────────────────────────────────────────────

impl ClassInfo {
    /// Return a mutable reference to the `LaravelMetadata`, creating it
    /// if absent.
    ///
    /// This is the preferred way to set Laravel-specific fields in tests
    /// and parsing code: `class.laravel_mut().casts_definitions = …;`
    pub fn laravel_mut(&mut self) -> &mut LaravelMetadata {
        self.laravel
            .get_or_insert_with(|| Box::new(LaravelMetadata::default()))
    }

    /// Return a reference to the `LaravelMetadata`, if present.
    pub fn laravel(&self) -> Option<&LaravelMetadata> {
        self.laravel.as_deref()
    }

    /// Look up the stored `name_offset` for a member by name and kind.
    ///
    /// Returns `Some(offset)` when the member exists and has a non-zero
    /// offset, or `None` otherwise.  The `kind` string should be one of
    /// `"method"`, `"property"`, or `"constant"`.
    pub(crate) fn member_name_offset(&self, name: &str, kind: &str) -> Option<u32> {
        let off = match kind {
            "method" => self
                .methods
                .iter()
                .find(|m| m.name == name)
                .map(|m| m.name_offset),
            "property" => self
                .properties
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.name_offset),
            "constant" => self
                .constants
                .iter()
                .find(|c| c.name == name)
                .map(|c| c.name_offset),
            _ => None,
        };
        off.filter(|&o| o > 0)
    }

    /// Push a `ClassInfo` into `results` only if no existing entry shares
    /// the same class name.  This is the single place where completion /
    /// resolution code deduplicates candidate classes.
    pub(crate) fn push_unique(results: &mut Vec<ClassInfo>, cls: ClassInfo) {
        if !results.iter().any(|c| c.name == cls.name) {
            results.push(cls);
        }
    }

    /// Extend `results` with entries from `new_classes`, skipping any whose
    /// name already appears in `results`.
    pub(crate) fn extend_unique(results: &mut Vec<ClassInfo>, new_classes: Vec<ClassInfo>) {
        for cls in new_classes {
            Self::push_unique(results, cls);
        }
    }
}

// ─── File Context ───────────────────────────────────────────────────────────

/// Cached per-file context retrieved from the `Backend` maps.
///
/// Bundles the three pieces of file-level metadata that almost every
/// handler needs: the parsed classes, the `use` statement import table,
/// and the declared namespace.  Constructed by
/// [`Backend::file_context`](crate::Backend) to replace the repeated
/// lock-and-unwrap boilerplate that was duplicated across completion,
/// definition, and implementation handlers.
pub(crate) struct FileContext {
    /// Classes extracted from the file's AST (from `ast_map`).
    pub classes: Vec<ClassInfo>,
    /// Import table mapping short names to fully-qualified names
    /// (from `use_map`).
    pub use_map: HashMap<String, String>,
    /// The file's declared namespace, if any (from `namespace_map`).
    pub namespace: Option<String>,
}

// ─── Eloquent Constants ─────────────────────────────────────────────────────

/// The fully-qualified name of the Eloquent Collection class.
///
/// Used by the `LaravelModelProvider` to detect and replace collection
/// return types when a model declares a custom collection class.
pub const ELOQUENT_COLLECTION_FQN: &str = "Illuminate\\Database\\Eloquent\\Collection";

// ─── Recursion Depth Limits ─────────────────────────────────────────────────
//
// Centralised constants for the maximum recursion depth allowed when
// walking inheritance chains, trait hierarchies, mixin graphs, and type
// alias resolution.  Defining them in one place ensures that the same
// limit is used consistently across the inheritance, definition, and
// completion modules.

/// Maximum depth when walking the `extends` parent chain
/// (class → parent → grandparent → …).
pub(crate) const MAX_INHERITANCE_DEPTH: u32 = 20;

/// Maximum depth when recursing into `use Trait` hierarchies
/// (a trait can itself `use` other traits).
pub(crate) const MAX_TRAIT_DEPTH: u32 = 20;

/// Maximum depth when recursing into `@mixin` class graphs.
pub(crate) const MAX_MIXIN_DEPTH: u32 = 10;

/// Maximum depth when resolving `@phpstan-type` / `@psalm-type` aliases
/// (an alias can reference another alias).
pub(crate) const MAX_ALIAS_DEPTH: u8 = 10;

//! Data types used throughout the PHPantomLSP server.
//!
//! This module contains all the "model" structs and enums that represent
//! extracted PHP information (classes, methods, properties, constants)
//! as well as completion-related types (AccessKind, CompletionTarget).

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
    /// Optional type hint string (e.g. "string", "int", "?Foo").
    pub type_hint: Option<String>,
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
    /// The parameters of the method.
    pub parameters: Vec<ParameterInfo>,
    /// Optional return type hint string (e.g. "void", "string", "?int").
    pub return_type: Option<String>,
    /// Whether the method is static.
    pub is_static: bool,
    /// Visibility of the method (public, protected, or private).
    pub visibility: Visibility,
}

/// Stores extracted property information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// The property name WITHOUT the `$` prefix (e.g. "name", "age").
    /// This matches PHP access syntax: `$this->name` not `$this->$name`.
    pub name: String,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
    /// Whether the property is static.
    pub is_static: bool,
    /// Visibility of the property (public, protected, or private).
    pub visibility: Visibility,
}

/// Stores extracted constant information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct ConstantInfo {
    /// The constant name (e.g. "MAX_SIZE", "STATUS_ACTIVE").
    pub name: String,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
    /// Visibility of the constant (public, protected, or private).
    pub visibility: Visibility,
}

/// Describes the access operator that triggered completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Completion triggered after `->` (instance access).
    Arrow,
    /// Completion triggered after `::` (static access).
    DoubleColon,
    /// Completion triggered after `parent::`.
    ///
    /// This is an oddball: it shows both static **and** instance methods
    /// (since PHP allows `parent::nonStaticMethod()` from a child class),
    /// plus constants and static properties — but excludes private members.
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

/// Stores extracted class information from a parsed PHP file.
/// All data is owned so we don't depend on the parser's arena lifetime.
#[derive(Debug, Clone)]
pub struct ClassInfo {
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
    /// The parent class name from the `extends` clause, if any.
    /// This is the raw name as written in source (e.g. "BaseClass", "Foo\\Bar").
    pub parent_class: Option<String>,
}

#![allow(dead_code)]

use phpantom_lsp::Backend;
use std::collections::HashMap;
use std::fs;

pub fn create_test_backend() -> Backend {
    Backend::new_test()
}

// Minimal PHP stubs for UnitEnum and BackedEnum so that tests exercising
// the "embedded stub" code-path work without `composer install`.
static UNIT_ENUM_STUB: &str = "\
<?php
interface UnitEnum
{
    public static function cases(): array;
    public readonly string $name;
}
";

static BACKED_ENUM_STUB: &str = "\
<?php
interface BackedEnum extends UnitEnum
{
    public static function from(int|string $value): static;
    public static function tryFrom(int|string $value): ?static;
    public readonly int|string $value;
}
";

// ─── Function stubs ─────────────────────────────────────────────────────────
// Minimal PHP stubs for built-in functions grouped by extension/category.

static ARRAY_FUNCTIONS_STUB: &str = "\
<?php
/**
 * @param callable|null $callback
 * @param array $array
 * @param array ...$arrays
 * @return array
 */
function array_map(?callable $callback, array $array, array ...$arrays): array {}

/**
 * @param array &$array
 * @return mixed
 */
function array_pop(array &$array): mixed {}

/**
 * @param array &$array
 * @param mixed ...$values
 * @return int
 */
function array_push(array &$array, mixed ...$values): int {}

/**
 * @param string|int $key
 * @param array $array
 * @return bool
 */
function array_key_exists(string|int $key, array $array): bool {}
";

static STRING_FUNCTIONS_STUB: &str = "\
<?php
/**
 * @param string $haystack
 * @param string $needle
 * @return bool
 */
function str_contains(string $haystack, string $needle): bool {}

/**
 * @param string $string
 * @param int $offset
 * @param int|null $length
 * @return string
 */
function substr(string $string, int $offset, ?int $length = null): string {}
";

static JSON_FUNCTIONS_STUB: &str = "\
<?php
/**
 * @param string $json
 * @param bool|null $associative
 * @param int $depth
 * @param int $flags
 * @return mixed
 */
function json_decode(string $json, ?bool $associative = null, int $depth = 512, int $flags = 0): mixed {}
";

static DATE_FUNCTIONS_STUB: &str = "\
<?php
/**
 * @param string|null $datetime
 * @param DateTimeZone|null $timezone
 * @return DateTime|false
 */
function date_create(?string $datetime = \"now\", ?DateTimeZone $timezone = null): DateTime|false {}
";

static SIMPLEXML_FUNCTIONS_STUB: &str = "\
<?php
/**
 * @param string $data
 * @param string|null $class_name
 * @param int $options
 * @param string $namespace_or_prefix
 * @param bool $is_prefix
 * @return SimpleXMLElement|false
 */
function simplexml_load_string(string $data, ?string $class_name = null, int $options = 0, string $namespace_or_prefix = \"\", bool $is_prefix = false): SimpleXMLElement|false {}
";

static PCRE_FUNCTIONS_STUB: &str = "\
<?php
/**
 * @param string $pattern
 * @param string $subject
 * @param array|null &$matches
 * @param int $flags
 * @param int $offset
 * @return int|false
 */
function preg_match(string $pattern, string $subject, ?array &$matches = null, int $flags = 0, int $offset = 0): int|false {}
";

// ─── Class stubs ────────────────────────────────────────────────────────────

static DATETIME_CLASS_STUB: &str = "\
<?php
class DateTime
{
    public function __construct(?string $datetime = \"now\", ?DateTimeZone $timezone = null) {}

    /**
     * @param string $format
     * @return string
     */
    public function format(string $format): string {}

    /**
     * @param string $modifier
     * @return DateTime|false
     */
    public function modify(string $modifier): DateTime|false {}

    /**
     * @return int
     */
    public function getTimestamp(): int {}

    /**
     * @param int $year
     * @param int $month
     * @param int $day
     * @return DateTime
     */
    public function setDate(int $year, int $month, int $day): DateTime {}

    /**
     * @param int $hour
     * @param int $minute
     * @param int $second
     * @param int $microsecond
     * @return DateTime
     */
    public function setTime(int $hour, int $minute, int $second = 0, int $microsecond = 0): DateTime {}
}
";

static SIMPLEXMLELEMENT_CLASS_STUB: &str = "\
<?php
class SimpleXMLElement
{
    /**
     * @param string $expression
     * @return array|false|null
     */
    public function xpath(string $expression): array|false|null {}

    /**
     * @param string|null $namespaceOrPrefix
     * @param bool $isPrefix
     * @return SimpleXMLElement|null
     */
    public function children(?string $namespaceOrPrefix = null, bool $isPrefix = false): ?SimpleXMLElement {}

    /**
     * @param string|null $namespaceOrPrefix
     * @param bool $isPrefix
     * @return SimpleXMLElement|null
     */
    public function attributes(?string $namespaceOrPrefix = null, bool $isPrefix = false): ?SimpleXMLElement {}

    /**
     * @param string $qualifiedName
     * @param string|null $value
     * @param string|null $namespace
     * @return SimpleXMLElement|null
     */
    public function addChild(string $qualifiedName, ?string $value = null, ?string $namespace = null): ?SimpleXMLElement {}

    /**
     * @return string
     */
    public function getName(): string {}
}
";

// ─── Exception class stubs ──────────────────────────────────────────────────

static EXCEPTION_CLASS_STUB: &str = "\
<?php
class Exception implements Throwable
{
    public function __construct(string $message = \"\", int $code = 0, ?Throwable $previous = null) {}

    /**
     * @return string
     */
    final public function getMessage(): string {}

    /**
     * @return int
     */
    final public function getCode(): int {}

    /**
     * @return string
     */
    final public function getFile(): string {}

    /**
     * @return int
     */
    final public function getLine(): int {}

    /**
     * @return array
     */
    final public function getTrace(): array {}

    /**
     * @return string
     */
    final public function getTraceAsString(): string {}

    /**
     * @return ?Throwable
     */
    final public function getPrevious(): ?Throwable {}

    /**
     * @return string
     */
    public function __toString(): string {}
}
";

static RUNTIME_EXCEPTION_CLASS_STUB: &str = "\
<?php
class RuntimeException extends Exception {}
";

// ─── Constant stubs ─────────────────────────────────────────────────────────

static CONSTANTS_STUB: &str = "\
<?php
define('PHP_EOL', \"\\n\");
define('PHP_INT_MAX', 9223372036854775807);
define('PHP_INT_MIN', -9223372036854775808);
define('PHP_MAJOR_VERSION', 8);
define('SORT_ASC', 4);
define('SORT_DESC', 3);
";

/// Create a test backend whose `stub_index` contains minimal `Exception`
/// and `RuntimeException` stubs.  This makes catch-variable tests fully
/// self-contained — they work without phpstorm-stubs installed.
pub fn create_test_backend_with_exception_stubs() -> Backend {
    let mut stubs: HashMap<&'static str, &'static str> = HashMap::new();
    stubs.insert("Exception", EXCEPTION_CLASS_STUB);
    stubs.insert("RuntimeException", RUNTIME_EXCEPTION_CLASS_STUB);
    Backend::new_test_with_stubs(stubs)
}

/// Create a test backend whose `stub_index` contains minimal `UnitEnum`
/// and `BackedEnum` stubs.  This makes "embedded stub" tests fully
/// self-contained — they no longer require a prior `composer install`.
pub fn create_test_backend_with_stubs() -> Backend {
    let mut stubs: HashMap<&'static str, &'static str> = HashMap::new();
    stubs.insert("UnitEnum", UNIT_ENUM_STUB);
    stubs.insert("BackedEnum", BACKED_ENUM_STUB);
    Backend::new_test_with_stubs(stubs)
}

/// Create a test backend with embedded PHP stubs for built-in functions,
/// classes, and constants.  This makes the stub-function tests fully
/// self-contained — they work whether or not phpstorm-stubs are installed.
pub fn create_test_backend_with_function_stubs() -> Backend {
    // ── Class stubs ──
    let mut class_stubs: HashMap<&'static str, &'static str> = HashMap::new();
    class_stubs.insert("DateTime", DATETIME_CLASS_STUB);
    class_stubs.insert("SimpleXMLElement", SIMPLEXMLELEMENT_CLASS_STUB);
    class_stubs.insert("UnitEnum", UNIT_ENUM_STUB);
    class_stubs.insert("BackedEnum", BACKED_ENUM_STUB);

    // ── Function stubs ──
    let mut function_stubs: HashMap<&'static str, &'static str> = HashMap::new();
    // Array functions (all point to the same source)
    function_stubs.insert("array_map", ARRAY_FUNCTIONS_STUB);
    function_stubs.insert("array_pop", ARRAY_FUNCTIONS_STUB);
    function_stubs.insert("array_push", ARRAY_FUNCTIONS_STUB);
    function_stubs.insert("array_key_exists", ARRAY_FUNCTIONS_STUB);
    // String functions
    function_stubs.insert("str_contains", STRING_FUNCTIONS_STUB);
    function_stubs.insert("substr", STRING_FUNCTIONS_STUB);
    // JSON functions
    function_stubs.insert("json_decode", JSON_FUNCTIONS_STUB);
    // Date functions
    function_stubs.insert("date_create", DATE_FUNCTIONS_STUB);
    // SimpleXML functions
    function_stubs.insert("simplexml_load_string", SIMPLEXML_FUNCTIONS_STUB);
    // PCRE functions
    function_stubs.insert("preg_match", PCRE_FUNCTIONS_STUB);

    // ── Constant stubs ──
    let mut constant_stubs: HashMap<&'static str, &'static str> = HashMap::new();
    constant_stubs.insert("PHP_EOL", CONSTANTS_STUB);
    constant_stubs.insert("PHP_INT_MAX", CONSTANTS_STUB);
    constant_stubs.insert("PHP_INT_MIN", CONSTANTS_STUB);
    constant_stubs.insert("PHP_MAJOR_VERSION", CONSTANTS_STUB);
    constant_stubs.insert("SORT_ASC", CONSTANTS_STUB);
    constant_stubs.insert("SORT_DESC", CONSTANTS_STUB);

    Backend::new_test_with_all_stubs(class_stubs, function_stubs, constant_stubs)
}

/// Helper: create a temp workspace with a composer.json and PHP files,
/// then return a Backend configured with that workspace root + PSR-4 mappings.
pub fn create_psr4_workspace(
    composer_json: &str,
    files: &[(&str, &str)],
) -> (Backend, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("composer.json"), composer_json)
        .expect("failed to write composer.json");
    for (rel_path, content) in files {
        let full = dir.path().join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("failed to create dirs");
        }
        fs::write(&full, content).expect("failed to write PHP file");
    }

    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);
    (backend, dir)
}

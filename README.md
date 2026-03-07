# PHPantom

A fast, lightweight PHP language server written in Rust. Uses only a few MB of RAM regardless of project size and is fully responsive in milliseconds. No indexing phase, no background workers, no waiting.

> [!NOTE]
> PHPantom is in active development. The core editing features are solid and used daily on production codebases. Extract Function and broader diagnostics are still on the roadmap.

## Features

PHPantom focuses on deep type intelligence. Here's how it compares:

| | PHPantom | Intelephense | PHP Tools | Phpactor | PHPStorm |
|---|---|---|---|---|---|
| Completion | ✅ | ✅ | ✅ | ✅ | ✅ |
| Auto-import | ✅ | 💰 | ✅ | ✅ | ✅ |
| `@mixin` completion | ✅ | 💰 | ✅ | ✅ | 🚧 |
| Generics / `@template` | ✅ | 🚧 | ✅ | 🚧 | ✅ |
| `@phpstan` annotations | ✅ | ❌ | 🚧 | 🚧 | 🚧 |
| Conditional return types | ✅ | ❌ | ✅ | 🚧 | ❌ |
| Laravel Eloquent | ✅ | ❌ | 🚧 | ❌ | 🧩 |
| Array shape inference | ✅ | ❌ | ✅ | 🚧 | 🚧 |
| Object shape completion | ✅ | ❌ | ✅ | ❌ | 🚧 |
| Closure param inference | ✅ | 🚧 | 🚧 | 🚧 | ❌ |
| Generator body types | ✅ | ❌ | 🚧 | ❌ | ❌ |
| Go-to-definition | ✅ | ✅ | ✅ | ✅ | ✅ |
| Go-to-implementation | ✅ | ✅ | ❌ | ✅ | ✅ |
| Hover | ✅ | ✅ | ✅ | ✅ | ✅ |
| Signature help | ✅ | ✅ | ✅ | ✅ | ✅ |
| Find references | ✅ | ✅ | ✅ | ✅ | ✅ |
| Diagnostics | ✅ | ✅ | ✅ | ✅ | ✅ |
| Rename / refactoring | ✅ | ✅ | ✅ | ✅ | ✅ |
| Time to ready | **10 ms** | 1 min 25 s | 3 min 17 s | 15 min 39 s | 19 min 38 s |
| RAM usage | **7 MB** | 520 MB | 3.9 GB | 498 MB | 2.0 GB |
| Disk cache | **0** | 45 MB | 0 | 4.1 GB | 551 MB |

<sub>Performance measured on a production codebase: 21K PHP files, 1.5M lines of code (vendor + application). 🚧 = partial support. 🧩 = requires plugin.</sub>

> **Want to verify?** Open [`example.php`](example.php) in your editor and trigger completion at the marked locations. It exercises every feature in the table, including edge cases where tools diverge.

## Context-Aware Intelligence

- **Smart PHPDoc completion.** `@throws` detects uncaught exceptions in the method body, `@param` pre-fills from the signature, and tags are filtered to context and never suggested twice.
- **Array shape inference.** Literal arrays offer key completion with no annotation. Nested shapes, spreads, and array functions like `array_map` preserve element types.
- **Closure parameter inference.** `$users->map(fn($u) => $u->name)` infers `$u` as `User` from the collection's generic context.
- **Generator body types.** `yield` and `$x = yield` resolve to the correct `TValue` and `TSend` from the generator's return annotation.
- **Conditional return types.** PHPStan-style conditional `@return` types resolve to the concrete branch at each call site.
- **Type aliases and shapes.** `@phpstan-type`, `@phpstan-import-type`, and `object{...}` shapes all resolve through to completions.
- **Laravel Eloquent.** Relationships, scopes, accessors, casts, and Builder chains resolve end-to-end. No Larastan, no ide-helper, no database access required.
- **Everything else you'd expect.** Generics, type narrowing, named arguments, destructuring, first-class callables, anonymous classes, `@deprecated` detection, and namespace segment drilling.

## Project Awareness

PHPantom understands Composer projects out of the box:

- **Autoloader-accurate results.** Completions and go-to-definition only surface classes that Composer's autoloader can actually load, avoiding false positives from internal, inaccessible, or duplicate vendor classes. The classmap is the source of truth, so you see exactly what your application can use.
- **PSR-4 autoloading.** Resolves classes across files on demand.
- **Classmap and file autoloading.** `autoload_classmap.php` and `autoload_files.php`.
- **Embedded PHP stubs** from [phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) bundled in the binary, no runtime downloads needed.
- **`require_once` discovery.** Functions from required files are available for completion.
- **Go-to-implementation.** Jump from an interface or abstract class to all concrete implementations. Scans open files, classmap, PSR-4 directories, and embedded stubs.

> [!IMPORTANT]
> Run `composer install -o` (or `composer dump-autoload -o`) in your project to generate the optimized autoload files PHPantom needs for cross-file class resolution.
>
> If your project doesn't use Composer, you can create a minimal `composer.json`:
> ```json
> { "autoload": { "classmap": ["src/"] } }
> ```
> Then run `composer dump-autoload -o`.

## Documentation

- **[Installation](docs/SETUP.md).** Editor-specific setup for Zed, Neovim, PHPStorm, and others.
- **[Building from Source](docs/BUILDING.md).** Build, test, and debug instructions.
- **[Architecture](docs/ARCHITECTURE.md).** Symbol resolution, stub loading, and inheritance merging.
- **[Contributing](docs/CONTRIBUTING.md)**
- **[Changelog](docs/CHANGELOG.md)**
- **[Benchmarks](https://ajenbo.github.io/phpantom_lsp/dev/bench/).** Completion latency tracked on every commit.
- **[Roadmap](docs/todo.md).** Planned features and domain-specific plans.

## Acknowledgements

PHPantom stands on the shoulders of:

- **[Mago](https://github.com/carthage-software/mago):** the PHP parser that powers all of PHPantom's AST analysis.
- **[PHPStan](https://phpstan.org/)** and **[Psalm](https://psalm.dev/):** whose combined work on static analysis for PHP transformed the language's type ecosystem. Generics, array shapes, conditional return types, assertion annotations: these tools pushed each other forward and pushed the community toward rigorous PHPDoc annotations that make a language server like this possible. PHPantom's author cut his teeth on PHPStan, which is why `@phpstan-*` annotations are a first-class citizen here.
- **[JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs):** type information for the entire PHP standard library, embedded directly into the binary.
- **[Phpactor](https://github.com/phpactor/phpactor):** the PHP language server whose comprehensive test suite and benchmark fixtures informed PHPantom's own test coverage. Many of PHPantom's type inference fixtures were adapted from Phpactor's reflection tests.

## License

MIT. See [LICENSE](LICENSE).

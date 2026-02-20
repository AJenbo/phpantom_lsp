# PHPantom

A fast, lightweight PHP language server written in Rust. Uses only a few MB of RAM regardless of project size and is fully responsive in milliseconds. No indexing phase, no background workers, no waiting.

> [!NOTE]
> PHPantom is in active development. Completion and go-to-definition are solid and used daily. More LSP features (hover, signature help, find references) are on the roadmap.

## Features

PHPantom focuses on completion and go-to-definition and aims to do them really well. Here's where it stands:

| | PHPantom | Intelephense | PHP Tools | Phpactor | PHPStorm |
|---|---|---|---|---|---|
| Completion | ✅ | ✅ | ✅ | ✅ | ✅ |
| Auto-import | ✅ | 💰 | ✅ | ✅ | ✅ |
| Go-to-definition | ✅ | ✅ | ✅ | ✅ | ✅ |
| Go-to-implementation | 🚧 | ✅ | ❌ | ✅ | ✅ |
| `@mixin` completion | ✅ | 💰 | ✅ | ✅ | 🚧 |
| `@phpstan-assert` narrowing | ✅ | ❌ | 🚧 | ❌ | 🚧 |
| Conditional return types | ✅ | ❌ | ✅ | ❌ | 🚧 |
| Array shape inference | ✅ | ❌ | ✅ | ❌ | 🚧 |
| Object shape completion | ✅ | ❌ | ✅ | ❌ | 🚧 |
| `@phpstan-type` aliases | ✅ | ❌ | 🚧 | ❌ | ❌ |
| Hover | ❌ | ✅ | ✅ | ✅ | ✅ |
| Signature help | ❌ | ✅ | ✅ | ✅ | ✅ |
| Find references | ❌ | ✅ | ✅ | ✅ | ✅ |
| Diagnostics | ❌ | ✅ | ✅ | ✅ | ✅ |
| Rename / refactoring | ❌ | 💰 | ✅ | ✅ | ✅ |
| Time to ready | **10 ms** | 1 min 25 s | 3 min 17 s | 15 min 39 s | 19 min 38 s |
| RAM usage | **7 MB** | 520 MB | 3.9 GB | 498 MB | 2.0 GB |
| Disk cache | **0** | 45 MB | 0 | 4.1 GB | 551 MB |

<sub>Performance measured on a production codebase: 21K PHP files, 1.5M lines of code (vendor + application).</sub>

> **Want to verify?** Open [`example.php`](example.php) in your editor and trigger completion at the marked locations. It exercises every feature in the table, including edge cases where tools diverge.

## Context-Aware Intelligence

- **Smart PHPDoc completion.** `@throws` detects uncaught exceptions in the method body, including those propagated from called methods. `@param` pre-fills with the name and type from the signature. Tags are filtered to context: `@var` only in property docblocks, `@param` only when there are undocumented parameters. Already-documented tags aren't suggested again.
- **Array shape inference from code.** `$config = ['host' => 'localhost', 'port' => 3306]` offers key completion with no annotation. Incremental `$config['key'] = ...` assignments extend the shape. Nested access chains resolve through shapes and generics. `array_filter`, `array_map`, `array_pop`, `current`, etc. preserve the element type instead of losing it to `array`.
- **Guard clause stacking.** Early return narrows subsequent code. Multiple guards stack to whittle a union down. Works in ternaries, `match(true)`, with `is_a()`, `assert()`.
- **Generic collection foreach.** Iterating `Collection<User>`, `Generator<int, Item>`, or a class with `@implements IteratorAggregate<int, User>` resolves the loop variable to the element type. Keys too.
- **Generics.** `@template` with type substitution through inheritance chains and at call sites.
- **Conditional return types.** PHPStan-style `@return ($param is class-string<T> ? T : mixed)` resolves to the concrete branch at each call site.
- **Everything else you'd expect.** Named arguments completion, destructuring with named keys, chained method calls in assignments, `@deprecated` detection.

## Project Awareness

PHPantom understands Composer projects out of the box:

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

## Getting Started

See **[docs/SETUP.md](docs/SETUP.md)** for editor-specific installation instructions (Zed, Neovim, and other editors).

## Building from Source

See **[docs/BUILDING.md](docs/BUILDING.md)** for build, test, and debug instructions.

## Contributing

See **[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)**.

## Architecture

For details on how symbol resolution, stub loading, and inheritance merging work, see **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

## Changelog

See **[docs/CHANGELOG.md](docs/CHANGELOG.md)** for a detailed history of each release.

## Roadmap

See **[docs/todo.md](docs/todo.md)** for the backlog of known gaps, missing LSP features, and planned improvements.

## Acknowledgements

PHPantom stands on the shoulders of:

- **[Mago](https://github.com/carthage-software/mago):** the PHP parser that powers all of PHPantom's AST analysis.
- **[PHPStan](https://phpstan.org/)** and **[Psalm](https://psalm.dev/):** whose combined work on static analysis for PHP transformed the language's type ecosystem. Generics, array shapes, conditional return types, assertion annotations: these tools pushed each other forward and pushed the community toward rigorous PHPDoc annotations that make a language server like this possible. PHPantom's author cut his teeth on PHPStan, which is why `@phpstan-*` annotations are a first-class citizen here.
- **[JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs):** type information for the entire PHP standard library, embedded directly into the binary.

## License

MIT. See [LICENSE](LICENSE).

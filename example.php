<?php

/**
 * PHP Showcase
 *
 * A single-file playground for every completion and go-to-definition feature.
 * Trigger completion after -> / :: / $, or Ctrl+Click for go-to-definition.
 *
 * Layout:
 *   1. DEMOS       — open any demo() method and try completion inside it
 *   2. SCAFFOLDING — supporting definitions (scroll past these)
 *
 * Priority:
 *   Basic everyday features → Laravel → Trivial (works everywhere) → Advanced
 */

namespace Demo {

use Exception;
use Stringable;
use Demo\UserProfile as Profile;


// ═══════════════════════════════════════════════════════════════════════════
//  DEMOS — open any demo() method and trigger completion inside
// ═══════════════════════════════════════════════════════════════════════════


// ── Auto-Import (completion) ────────────────────────────────────────────────
// Try: type `new DateT` and accept `DateTime`. The `use DateTime;` statement
// is inserted between `use Exception;` and `use Stringable;` above to
// maintain alphabetical order.
//
// The `use Exception;` import above occupies the short name "Exception".
// Try: type `throw new pq\Exception()` and accept — the auto-import inserts
// `\pq\Exception` at the usage site instead of a conflicting `use` statement.

// ── Namespace Segment Completion ────────────────────────────────────────────
// Try: erase the class name after `use Demo\` and trigger completion to see
// namespace segments (module/folder icon) alongside class names.

// ── Namespaced Function Completion ──────────────────────────────────────────
// Try: type `use function parse_file` and accept to get
// `use function ast\parse_file;`


// ── Instance Completion ─────────────────────────────────────────────────────

class InstanceCompletionDemo
{
    public function demo(): void
    {
        $zoo = new Zoo();

        $zoo->aardvark();            // own method
        $zoo->baboon;                // own property
        $zoo->buffalo;               // constructor-promoted property
        $zoo->cheetah;               // readonly promoted (from base)
        $zoo->dingo();               // trait method
        $zoo->elephant('Hi');        // trait method
        $zoo->falcon();              // inherited from parent
        $zoo->gorilla;               // @property (own class)
        $zoo->hyena('x');            // @method (own class)
        $zoo->iguana;                // @property-read (interface)
        $zoo->jaguar();              // @method (interface)
        // MUST NOT appear: $keeper (protected), $ceo (private), nocturnal() (private)
    }
}


// ── Method & Property Chaining ──────────────────────────────────────────────

class ChainingDemo
{
    public function demo(): void
    {
        $studio = new ScaffoldingChainingDemo();

        // Fluent method chains — MUST NOT appear: calibrate() (protected)
        $studio->brush->setSize('large')->setStyle('pointed')->stroke();

        // Return type chains
        $studio->brush->getCanvas()->title();

        // Variable → method chain
        $canvas = $studio->brush->getCanvas();
        $canvas->getBrush()->stroke();

        // Deep property chain
        $studio->canvas->easel->material;
        $studio->canvas->easel->height();

        // Null-safe chaining
        $maybe = Brush::find(1);
        $maybe?->getCanvas()?->title();

        // Multi-line method chains
        $studio->brush->setSize('large')
            ->setStyle('pointed')
            ->stroke();

        // Variable assigned from chain
        $directBrush = $studio->brush->getCanvas()->getBrush();
        $directBrush->stroke();

        // (new Class())->method()
        $fromNew = (new Canvas())->getBrush();
        $fromNew->stroke();

        // Intermediate variable from property access
        $easel = (new Canvas())->easel;
        $easel->material;
    }
}


// ── @var Docblock Override ──────────────────────────────────────────────────

class VarDocblockDemo
{
    public function demo(): void
    {
        /** @var Pencil $inlineHinted */
        $inlineHinted = getUnknownValue();
        $inlineHinted->sketch();                  // with explicit variable name

        /** @var Pen */
        $hinted = getUnknownValue();
        $hinted->write();                         // without variable name (PHPStorm fails this)
    }
}


// ── Return Type Resolution ──────────────────────────────────────────────────

class ReturnTypeDemo
{
    public function demo(): void
    {
        $made = Pen::make();                      // static return type → Pen
        $made->write();

        $marker = Marker::make();                 // static on subclass → Marker
        $marker->highlight();                     // resolves to Marker, not Pen

        $fluent = $marker->rename('Bold');         // rename returns static → Marker
        $fluent->highlight();                     // chained static stays on the subclass

        $created = makePen();
        $created->write();                        // function return type
        // MUST NOT appear: refill() (private)

        $found = pickPenOrPencil();               // Pen|Pencil union
        $found->label();                          // available on both types
    }
}


// ── Type Narrowing ──────────────────────────────────────────────────────────

class TypeNarrowingDemo
{
    public function demo(): void
    {
        $specimen = pickRockOrBanana();           // Rock|Banana
        if ($specimen instanceof Rock) {
            $specimen->crush();                   // narrowed to Rock
            // MUST NOT appear: peel() (Banana only)
        } else {
            $specimen->peel();                    // narrowed to Banana (else branch)
            // MUST NOT appear: crush() (Rock only)
        }

        if (!$specimen instanceof Rock) {
            $specimen->peel();                    // negated instanceof
        }

        $unknown = getUnknownValue();
        if (is_a($unknown, Rock::class)) {
            $unknown->crush();                    // is_a() narrowing
        }

        $target = getUnknownValue();
        assert($target instanceof Banana);
        $target->peel();                          // assert() narrowing
    }
}


// ── Custom Assert Narrowing ─────────────────────────────────────────────────

class AssertNarrowingDemo
{
    public function demo(): void
    {
        $unknown = getUnknownValue();
        assertRock($unknown);                     // @phpstan-assert Rock $value
        $unknown->crush();

        $sample = pickRockOrBanana();
        if (isRock($sample)) {                    // @phpstan-assert-if-true Rock
            $sample->crush();
        } else {
            $sample->peel();
        }

        $maybe = pickRockOrBanana();
        if (isNotRock($maybe)) {                  // @phpstan-assert-if-false Rock
            $maybe->peel();
        } else {
            $maybe->crush();
        }
    }
}


// ── Guard Clause Narrowing (Early Return / Throw) ──────────────────────────

class GuardClauseDemo
{
    public function demo(): void
    {
        $subject = pickRockOrBanana();            // Rock|Banana
        if (!$subject instanceof Banana) {
            return;                               // early return — guard clause
        }
        $subject->peel();                         // narrowed to Banana after guard

        $candidate = pickRockOrBanana();          // Rock|Banana
        if ($candidate instanceof Rock) {
            throw new Exception('no rocks');       // early throw — guard clause
        }
        $candidate->peel();                       // narrowed to Banana (Rock excluded)

        $unknown = getUnknownValue();
        if (!$unknown instanceof Rock) return;    // single-statement guard (no braces)
        $unknown->crush();                        // narrowed to Rock
    }
}


// ── in_array Strict-Mode Narrowing ─────────────────────────────────────────

class InArrayNarrowingDemo
{
    /**
     * @param Rock|Banana $item
     * @param list<Rock> $rocks
     */
    public function demo($item, array $rocks): void
    {
        if (in_array($item, $rocks, true)) {
            $item->crush();                       // narrowed to Rock
            // MUST NOT appear: peel() (Banana only)
        } else {
            $item->peel();                        // excluded Rock → Banana
            // MUST NOT appear: crush() (Rock only)
        }

        // Guard clause with in_array
        $specimen = pickRockOrBanana();           // Rock|Banana
        if (!in_array($specimen, $rocks, true)) {
            return;
        }
        $specimen->crush();                       // narrowed to Rock after guard
    }
}


// ── Generics (@template / @extends) ────────────────────────────────────────

class GenericsDemo
{
    public function demo(): void
    {
        $repo = new PenRepository();
        $repo->find(1)->write();                  // Repository<Pen>::find() → Pen
        $repo->findOrNull(1)?->write();           // ?Pen

        $pens = new PenCollection();              // TypedCollection<int, Pen>
        $pens->first()->write();
        // MUST NOT appear: refill() (private on Pen)
        $pens->thickOnly();                       // own method on subclass

        $cachingRepo = new CachingPenRepository();
        $cachingRepo->find(1)->write();           // grandparent generics

        $responses = new ResponseCollection();    // @phpstan-extends variant
        $responses->first()->getStatusCode();
    }
}


// ── Conditional Return Types ────────────────────────────────────────────────

class ConditionalReturnDemo
{
    public function demo(): void
    {
        $container = new Container();
        $resolved = $container->make(Pen::class);
        $resolved->write();                       // class-string<T> → T

        $appPen = app(Pen::class);                // conditional on standalone function
        $appPen->write();
    }
}


// ── Method-Level @template ──────────────────────────────────────────────────

class MethodTemplateDemo
{
    public function demo(): void
    {
        $locator = new ServiceLocator();
        $locator->get(Pen::class)->write();               // class-string<T> → T

        Factory::create(Pen::class)->write();             // static @template
        resolve(Marker::class)->highlight();              // function @template

        $mapper = new ObjectMapper();
        $mapped = $mapper->wrap(new Pen());
        $mapped->first();                         // → Pen (T resolved from argument)

        $mapper->wrap(new Product())->first()->getPrice(); // new expression arg → Product
    }
}


// ── Trait Generic Substitution ──────────────────────────────────────────────

class TraitGenericDemo
{
    public function demo(): void
    {
        Product::factory()->create();             // @use HasFactory<UserFactory> → UserFactory
        Product::factory()->count(5)->make();     // count() returns static, make() returns Product

        $idx = new PenIndex();                    // @use Indexable<int, Pen>
        $idx->get()->write();                     // TValue → Pen
    }
}


// ── Foreach & Array Access ──────────────────────────────────────────────────

class ForeachArrayAccessDemo
{
    public function demo(): void
    {
        /** @var list<Pen> $members */
        $members = getUnknownValue();
        foreach ($members as $member) {
            $member->write();                     // element type from list<Pen>
        }
        $members[0]->color();                     // array access element type

        /** @var array<int, Pen> */
        $annotated = [];                          // @var without variable name
        $annotated[0]->write();                   // type from next-line annotation

        $inferred = [new Pen(), new Marker()];
        $inferred[0]->write();                    // element type inferred from literal
    }
}


// ── Array Destructuring ────────────────────────────────────────────────────

class ArrayDestructuringDemo
{
    public function demo(): void
    {
        /** @var list<Pen> */
        [$first, $second] = getUnknownValue();
        $first->write();                          // destructured element type
    }
}


// ── Array Shapes ────────────────────────────────────────────────────────────

class ArrayShapeDemo
{
    public function demo(): void
    {
        // Literal array shape — key completion and value types
        $config = ['host' => 'localhost', 'port' => 3306, 'tool' => new Pen()];
        $config[''];                              // Try: key completion: host, port, tool
        $config['tool']->write();                 // value type → Pen

        // Annotated shape
        /** @var array{first: Pen, second: Pencil} $pair */
        $pair = getUnknownValue();
        $pair['first']->write();
        $pair['second']->sketch();

        // Shape from function return type
        $cfg = getAppConfig();
        $cfg['logger']->write();
    }
}


// ── Object Shapes ───────────────────────────────────────────────────────────

class ObjectShapeDemo
{
    public function demo(): void
    {
        /** @var object{title: string, score: float} $item */
        $item = getUnknownValue();
        $item->title;                             // Ctrl+Click → jumps to `title:` in docblock above
        $item->score;                             // Ctrl+Click → jumps to `score:` in docblock above
    }
}


// ── Spread Operator Type Tracking ───────────────────────────────────────────

class SpreadOperatorDemo
{
    public function demo(): void
    {
        /** @var list<Pen> */
        $penList = [];
        /** @var list<Pencil> */
        $pencilList = [];

        $allPens = [...$penList];
        $allPens[0]->write();                     // resolves Pen from spread

        $everything = [...$penList, ...$pencilList];
        $everything[0]->label();                  // union: Pen|Pencil from multiple spreads
    }
}


// ── Clone Expression ────────────────────────────────────────────────────────

class CloneDemo
{
    public function demo(): void
    {
        $pen = new Pen('blue');
        $copy = clone $pen;
        $copy->write();                           // preserves Pen type
    }
}


// ── Class-String Variable Static Access ─────────────────────────────────────

class ClassStringStaticDemo
{
    public function demo(): void
    {
        $cls = Pen::class;
        $cls::make();                             // static method from Pen
    }
}


// ── Ambiguous Variables ─────────────────────────────────────────────────────

class AmbiguousVariableDemo
{
    public function demo(): void
    {
        if (rand(0, 1)) {
            $ambiguous = new Lamp();
        } else {
            $ambiguous = new Faucet();
        }
        $ambiguous->turnOff();                    // available on both branches
        $ambiguous->dim();                        // available on Lamp branches
        $ambiguous->drip();                       // available on Faucet branches
    }
}


// ── Parenthesized Assignment ────────────────────────────────────────────────

class ParenthesizedAssignmentDemo
{
    public function demo(): void
    {
        $parenPen = (new Pen('red'));
        $parenPen->write();                       // resolves through parentheses
    }
}


// ── String Interpolation ────────────────────────────────────────────────────

class StringInterpolationDemo
{
    public function demo(): void
    {
        $pen = new Pen('blue');
        echo "Ink is {$pen->color()}";             // brace interpolation — full completion
        echo "Tool: $pen->ink";                    // simple interpolation
        echo 'no $pen-> here';                     // single-quoted — suppressed
    }
}


// ── Foreach over Generic Collection Classes ─────────────────────────────────

class CollectionForeachDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingCollectionForeach();

        // From method return type
        foreach ($src->allMembers() as $user) {
            $user->getName();             // via method return type → collection generics
        }

        // From new instance
        $items = new UserEloquentCollection();
        foreach ($items as $item) {
            $item->getEmail();            // resolves to User via @extends generics
        }

        // From property type
        foreach ($src->members as $user) {
            $user->getEmail();            // via property type → collection generics
        }

        // From variable
        $collection = $src->allMembers();
        foreach ($collection as $user) {
            $user->getName();             // via variable assignment scanning
        }
    }
}


// ── Type Aliases (@phpstan-type / @phpstan-import-type) ─────────────────────

/**
 * @phpstan-type UserData array{name: string, email: string, age: int}
 * @phpstan-type StatusInfo array{code: int, label: string}
 */
class TypeAliasDemo
{
    public function demo(): void
    {
        $data = $this->getUserData();
        $data['name'];                    // @phpstan-type → array shape key completion

        $status = $this->getStatus();
        $status['label'];                 // StatusInfo alias → array shape keys
    }

    /** @return UserData */
    public function getUserData(): array
    {
        return ['name' => 'Alice', 'email' => 'alice@example.com', 'age' => 30];
    }

    /** @return StatusInfo */
    public function getStatus(): array
    {
        return ['code' => 200, 'label' => 'OK'];
    }
}

/**
 * @phpstan-import-type UserData from TypeAliasDemo
 * @phpstan-import-type StatusInfo from TypeAliasDemo as AliasedStatus
 */
class TypeAliasImportDemo
{
    public function demo(): void
    {
        $user = $this->fetchUser();
        $user['email'];                   // imported UserData → array shape keys

        $status = $this->fetchStatus();
        $status['code'];                  // AliasedStatus → StatusInfo → array shape keys
    }

    /** @return UserData */
    public function fetchUser(): array
    {
        return ['name' => 'Bob', 'email' => 'bob@example.com', 'age' => 25];
    }

    /** @return AliasedStatus */
    public function fetchStatus(): array
    {
        return ['code' => 404, 'label' => 'Not Found'];
    }
}


// ── Multi-line @return & Broken Docblock Recovery ───────────────────────────

class BrokenDocblockDemo
{
    public function demo(): void
    {
        $collection = collect([]);
        $collection->groupBy('key');             // multi-line @return resolves correctly

        $recovered = (new BrokenDocRecovery())->broken();
        $recovered->working();                   // recovers `static` from broken @return static<
    }
}


// ── Callable / Closure Variable Invocation ──────────────────────────────────

class ClosureInvocationDemo
{
    public function demo(): void
    {
        // Closure literal with native return type hint
        $makePen = function(): Pen { return new Pen(); };
        $makePen()->write();                      // resolves Pen from closure return type

        // Arrow function literal
        $makePencil = fn(): Pencil => new Pencil();
        $makePencil()->sketch();                  // arrow fn return type

        // Docblock callable annotation
        /** @var \Closure(): Pencil $supplier */
        $supplier = getUnknownValue();
        $supplier()->sharpen();                   // @var Closure() annotation

        // Chaining after callable invocation
        $builder = function(): Pen { return new Pen(); };
        $builder()->rename('Bold')->write();      // chain after $fn()

        // Variable assigned from callable invocation
        $fromClosure = $makePen();
        $fromClosure->write();                    // $result = $fn() resolves return type
    }
}


// ── Anonymous Classes ───────────────────────────────────────────────────────

class AnonymousClassDemo
{
    public function demo(): object
    {
        return new class extends Pen {
            public string $brand;
            public function cap(): string { return ''; }
            public function demo() {
                $this->cap();                    // own method
                $this->brand;                    // own property
                $this->write();                  // inherited from Pen
                // MUST NOT appear: refill() (private — not inherited)
            }
        };
    }
}


// ── Match / Ternary / Null-Coalescing Type Accumulation ─────────────────────

class ExpressionTypeDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingExpressionType();

        // Null-coalescing
        $fallback = $src->backup ?? $src->primary;
        $fallback->getStatusCode();       // Response method

        // Match expression — shared members sort above branch-only members
        $service = match (rand(0, 1)) {
            0 => new ElasticProductReviewIndexService(),
            1 => new ElasticBrandIndexService(),
        };
        $service->index();                // on both — sorted first
        $service->reindex();              // one branch only — sorted after

        // Ternary
        $svc = rand(0, 1)
            ? new ElasticProductReviewIndexService()
            : new ElasticBrandIndexService();
        $svc->index();                    // on both — sorted first
    }
}


// ── Switch Statement Type Tracking ──────────────────────────────────────────

class SwitchDemo
{
    public function demo(string $type): void
    {
        switch ($type) {
            case 'reviews':
                $service = new ElasticProductReviewIndexService();
                break;
            case 'brands':
                $service = new ElasticBrandIndexService();
                break;
        }
        $service->index();                // on both classes
    }
}


// ── Array & Object Shapes in Methods ────────────────────────────────────────

class ShapeMethodDemo
{
    public function demo(): void
    {
        $data = $this->getToolKit();
        $data['pen']->write();            // Pen
        $data['pencil']->sketch();        // Pencil

        // Nested annotated shape
        /** @var array{meta: array{page: int, total: int}, items: list<Pen>} $response */
        $response = getUnknownValue();
        $response['meta']['page'];        // nested shape key
        $response['items'][0]->write();   // list element type

        // Nested keys inferred from literal — no annotation needed
        $config = ['db' => ['host' => 'localhost', 'port' => 3306], 'debug' => true];
        $config['db']['host'];            // Try: delete 'host' and trigger completion

        // Object shapes
        $profile = $this->getProfile();
        $profile->name;                   // Ctrl+Click → jumps to `name:` in @return docblock

        $result = $this->getResult();
        $result->tool->write();           // Ctrl+Click `tool` → jumps to `tool:` in @return docblock
        $result->meta->page;              // Ctrl+Click `meta` → jumps to `meta:` in @return docblock
    }

    /** @return array{pen: Pen, pencil: Pencil, active: bool} */
    public function getToolKit(): array { return []; }

    /** @return object{name: string, age: int, active: bool} */
    public function getProfile(): object { return (object) []; }

    /** @return object{tool: Pen, meta: object{page: int, total: int}} */
    public function getResult(): object { return (object) []; }

    /** @param array{host: string, port: int, tool: Pen} $config */
    public function fromParam(array $config): void
    {
        $config['host'];                  // string
        $config['tool']->write();         // Pen
    }
}


// ── Named Key Destructuring from Array Shapes ───────────────────────────────

class DestructuringShapeDemo
{
    public function demo(): void
    {
        // Named key from method return
        ['pen' => $pen, 'pencil' => $pencil] = $this->getToolKit();
        $pen->write();                    // Pen from 'pen' key
        $pencil->sketch();                // Pencil from 'pencil' key

        // Named key from @var annotated variable
        /** @var array{pen: Pen, pencil: Pencil, active: bool} $data */
        $data = getUnknownValue();
        ['pen' => $myPen, 'pencil' => $myPencil] = $data;
        $myPen->write();                  // Pen from 'pen' key
        $myPencil->sketch();              // Pencil from 'pencil' key

        // Positional from shape
        /** @var array{Pen, Pencil} $pair */
        $pair = getUnknownValue();
        [$first, $second] = $pair;
        $first->write();                  // Pen (positional index 0)
        $second->sketch();                // Pencil (positional index 1)

        // list() syntax
        /** @var array{recipe: Recipe, servings: int} $meal */
        $meal = getUnknownValue();
        list('recipe' => $recipe) = $meal;
        $recipe->ingredients;             // Recipe from 'recipe' key
    }

    /** @return array{pen: Pen, pencil: Pencil, count: int} */
    public function getToolKit(): array { return []; }
}


// ── Generic Context Preservation ────────────────────────────────────────────

class GenericContextDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingGenericContext();

        $src->chest->unwrap()->open();             // Box<Gift>::unwrap() → Gift
        $src->display()->first()->open();          // TypedCollection<int, Gift>::first() → Gift
    }
}


// ── Foreach, Key Types, and Destructuring ───────────────────────────────────

class IterationDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingIteration();

        // From method
        foreach ($src->allPens() as $pen) {
            $pen->write();                // list<Pen> → Pen
        }

        // From property
        foreach ($src->batch as $pen) {
            $pen->write();
        }

        // Key types
        foreach ($src->crossRef() as $pen => $pencil) {
            $pen->write();                // Pen (key type)
            $pencil->sketch();            // Pencil (value type)
        }

        // WeakMap keys
        /** @var \WeakMap<Pen, Pencil> $mapping */
        $mapping = new \WeakMap();
        foreach ($mapping as $pen => $pencil) {
            $pen->write();                // key: Pen
            $pencil->sketch();            // value: Pencil
        }

        // Destructuring
        [$first, $second] = $src->allPens();
        $first->write();                  // destructured element type
    }
}


// ── Array Function Type Preservation ────────────────────────────────────────

class ArrayFuncDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingArrayFunc();

        $active = array_filter($src->members, fn(Pen $pen) => $pen->color() === 'blue');
        $active[0]->write();              // Pen preserved through array_filter

        $vals = array_values($src->members);
        $vals[0]->write();                // Pen preserved through array_values

        $pens = $src->roster();
        $last = array_pop($pens);
        $last->write();                   // single Pen from array_pop

        $cur = current($src->members);
        $cur->write();                    // Pen from current()

        end($src->members)->write();      // inline end() without variable

        foreach (array_filter($src->members, fn(Pen $pen) => true) as $pen) {
            $pen->color();                // Pen preserved in foreach
        }

        $mapped = array_map(fn($pen) => $pen, $src->members);
        $mapped[0]->write();              // Pen from array_map fallback
    }
}


// ── @throws Completion and Catch Variable Types ─────────────────────────────

class ExceptionDemo extends ScaffoldingException
{
    public function demo(): void
    {
        try {
            $this->riskyOperation();
        } catch (ValidationException $e) {
            $e->getMessage();             // catch variable resolves exception type
        }
    }

    /**
     * Typing `@` in this docblock suggests @throws for each uncaught exception.
     *
     * @throws NotFoundException
     * @throws ValidationException
     */
    public function findOrFail(int $id): array
    {
        if ($id < 0) {
            throw new ValidationException('ID must be positive');
        }
        $result = $this->lookup($id);
        if ($result === null) {
            throw new NotFoundException('Record not found');
        }
        return $result;
    }

    /**
     * Caught exceptions are filtered out of @throws suggestions.
     *
     * @throws AuthorizationException
     */
    public function safeOperation(): void
    {
        try {
            throw new \RuntimeException('transient error');
        } catch (\RuntimeException $e) {
            // caught — not suggested
        }
        throw new AuthorizationException('Forbidden');
    }

    /**
     * Called method's @throws propagate to the caller.
     *
     * @throws AuthorizationException
     */
    public function delegatedWork(): void
    {
        $this->safeOperation();
    }
}


// ── Constructor @param → Promoted Property Override ─────────────────────────

class ParamOverrideDemo
{
    public function demo(): void
    {
        foreach ($this->ingredients as $ingredient) {
            $ingredient->name;              // Ingredient::$name
            $ingredient->format();          // Ingredient::format()
        }
        $this->recipe->title;               // Recipe::$title
    }

    /**
     * @param list<Ingredient> $ingredients
     * @param Recipe $recipe
     */
    public function __construct(
        public array $ingredients,          // @param overrides to list<Ingredient>
        public object $recipe,              // @param overrides to Recipe
    ) {}
}


// ── Generator / Iterable Yield Type Resolution ─────────────────────────────

class GeneratorDemo
{
    public function demo(): void
    {
        // Generator<int, Pen> — value is 2nd param (Pen)
        foreach ($this->getPens() as $pen) {
            $pen->write();                // resolves to Pen
        }

        // Generator<int, Pencil, mixed, Pen> — value is still 2nd param (Pencil)
        foreach ($this->processPencils() as $pencil) {
            $pencil->sketch();            // Pencil (2nd param), not Pen (4th)
        }

        // @var annotated generator
        /** @var \Generator<int, Pen> $gen */
        $gen = $this->getPens();
        foreach ($gen as $pen) {
            $pen->write();                // Generator<int, Pen> → Pen
        }

        // iterable<Pen> — single param is the value type
        foreach ($this->iterablePens() as $pen) {
            $pen->write();
        }

        // Method chain through generator element
        foreach ($this->getPens() as $pen) {
            $pen->rename('Bold')->color();
        }
    }

    /** @return \Generator<int, Pen> */
    public function getPens(): \Generator
    {
        yield new Pen();
    }

    /** @return \Generator<int, Pencil, mixed, Pen> */
    public function processPencils(): \Generator
    {
        yield new Pencil();
    }

    /** @return iterable<Pen> */
    public function iterablePens(): iterable
    {
        return [];
    }

    /**
     * @param \Generator<int, Pencil> $pencils
     */
    public function foreachGeneratorParam(\Generator $pencils): void
    {
        foreach ($pencils as $pencil) {
            $pencil->sketch();            // @param overrides native \Generator type
        }
    }
}


// ── Generator Yield Type Inference Inside Bodies ────────────────────────────

class GeneratorYieldDemo
{
    /** @return \Generator<int, Pen> */
    public function findAll(): \Generator
    {
        // yield $var infers the variable as Pen (TValue = 2nd param)
        yield $pen;
        $pen->write();                    // resolves to Pen

        yield 0 => $anotherPen;
        $anotherPen->color();             // key => value yields also work
    }

    /** @return \Generator<int, Pen> */
    public function yieldInsideControlFlow(): \Generator
    {
        if (true) {
            yield $pen;
            $pen->write();                // resolves inside control flow blocks
        }
    }

    /** @return \Generator<int, Pen> */
    public function chainingThroughYieldInferred(): \Generator
    {
        yield $pen;
        $pen->rename('Bold')->color();    // chains through inferred type
    }

    /** @return \Generator<int, string, Pencil, void> */
    public function coroutine(): \Generator
    {
        // TSend inference: $var = yield assigns the 3rd type param
        $pencil = yield 'ready';
        $pencil->sketch();                // resolves to Pencil (TSend)
    }

    /** @return \Generator<int, string, Pencil, void> */
    public function tsendInsideNestedBlocks(): \Generator
    {
        while (true) {
            if (true) {
                $pencil = yield 'waiting';
                $pencil->sketch();        // resolves inside nested blocks
            }
        }
    }
}


// ── Template Parameter Bounds ───────────────────────────────────────────────

/**
 * @template-covariant TNode of AstNode
 */
class TemplateBoundsDemo
{
    public function demo(): void
    {
        $this->node->getChildren();       // resolves via TNode's bound: AstNode
        $this->node->getParent();
    }

    /** @var TNode */
    public $node;

    /** @param TNode $node */
    public function __construct(AstNode $node)
    {
        $this->node = $node;
    }
}


// ── Match Class-String Forwarding to Conditional Return Types ───────────────

class MatchClassStringDemo
{
    public function demo(): void
    {
        $container = new Container();

        // Match expression → class-string → conditional return
        $requestType = match (rand(0, 1)) {
            0 => ElasticProductReviewIndexService::class,
            1 => ElasticBrandIndexService::class,
        };
        $requestBody = $container->make($requestType);
        $requestBody->index();            // on both classes
        $requestBody->reindex();          // ElasticProductReviewIndexService only

        // Standalone function with @template
        $resolved = resolve($requestType);
        $resolved->index();               // on both classes

        // Inline chain
        $container->make($requestType)->index();

        // Simple class-string variable
        $cls = Pen::class;
        $pen = $container->make($cls);
        $pen->write();                    // resolves through simple $cls variable

        // Ternary class-string
        $ternary = rand(0, 1) ? Pen::class : Pencil::class;
        $obj = $container->make($ternary);
        $obj->label();                    // shared by both types
    }
}


// ═══════════════════════════════════════════════════════════════════════════
//  LARAVEL — Eloquent features that other editors struggle with
// ═══════════════════════════════════════════════════════════════════════════


// ── Eloquent Virtual Properties ─────────────────────────────────────────────
// Alphabetical — every property a through v should appear in order.
// Trigger completion on `$bakery->` and scan the list.

class EloquentPropertyDemo
{
    public function demo(): void
    {
        $bakery = new Bakery();

        $bakery->apricot;             // $casts 'boolean'           → bool
        $bakery->baguettes;           // relationship HasMany       → Collection<Loaf>
        $bakery->baguettes_count;     // relationship count         → int
        $bakery->croissant;           // $attributes default        → string
        $bakery->dough_temp;          // $casts 'float'             → float
        $bakery->egg_count;           // $attributes default        → int
        $bakery->flour;               // $fillable (no cast/attr)   → mixed
        $bakery->fresh();             // #[Scope] method            → Builder
        $bakery->gluten_free;         // $attributes default        → bool
        $bakery->headBaker;           // relationship HasOne        → Baker
        $bakery->head_baker_count;    // relationship count         → int
        $bakery->icing;               // $casts custom class        → ?Frosting
        $bakery->jam_flavor;          // $casts enum                → JamFlavor
        $bakery->kitchen_id;          // $guarded (no cast/attr)    → mixed
        $bakery->loaf_name;           // legacy accessor            → string
        $bakery->masterRecipe;        // relationship BelongsToMany → Collection<BakeryRecipe>
        $bakery->master_recipe_count; // relationship count         → int
        $bakery->notes;               // $casts 'array'             → array
        $bakery->oven_code;           // $hidden (no cast/attr)     → mixed
        $bakery->proved_at;           // $casts 'datetime'          → \Carbon\Carbon
        $bakery->quality;             // casts() method 'float'     → float
        $bakery->rye_blend;           // $visible (no cast/attr)    → mixed
        $bakery->sprinkle;            // modern accessor Attribute  → string
        $bakery->topping('choc');     // scope method               → Builder
        $bakery->unbaked();           // scope method               → Builder
        $bakery->vendor;              // body-inferred morphTo      → Model
        $bakery->vendor_count;        // relationship count         → int
        // MUST NOT appear: secret_ingredient (private $attributes field)
    }
}


// ── Eloquent Query Builder ──────────────────────────────────────────────────

class EloquentQueryDemo
{
    public function demo(): void
    {
        // Builder-as-static forwarding
        BlogAuthor::where('active', true);
        BlogAuthor::where('active', 1)->get();     // → Collection<BlogAuthor>
        BlogAuthor::where('active', 1)->first();   // → BlogAuthor|null
        BlogAuthor::orderBy('name')->limit(10)->get();
        BlogAuthor::whereIn('id', [1, 2])->groupBy('genre')->get();
        BlogAuthor::where('active', 1)->first()->profile->getBio();

        // Scope methods — instance and static
        $author = new BlogAuthor();
        $author->active();
        $author->ofGenre('fiction');
        BlogAuthor::active();
        BlogAuthor::ofGenre('fiction');

        // Scopes on Builder instances (convention and #[Scope] attribute)
        BlogAuthor::where('active', 1)->active()->ofGenre('sci-fi')->get();
        Bakery::where('open', true)->fresh()->get();
        $query = BlogAuthor::where('genre', 'fiction');
        $query->active();
        $query->orderBy('name')->get();
    }
}


// ── Custom Eloquent Collections ─────────────────────────────────────────────

class CustomCollectionDemo
{
    public function demo(): void
    {
        // Builder chain → custom collection via #[CollectedBy]
        $reviews = Review::where('published', true)->get();
        $reviews->topRated();             // custom method from ReviewCollection
        $reviews->averageRating();        // custom method from ReviewCollection
        $reviews->first();                // inherited — returns Review|null

        // Relationship properties also use the custom collection
        $review = new Review();
        $review->replies->topRated();     // HasMany<Review> → ReviewCollection
    }
}


// ── Closure Parameter Inference ─────────────────────────────────────────────

class ClosureParamInferenceDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingClosureParamInference();

        // $p is inferred as Pen from map's callable(TValue, TKey) signature
        $src->items->map(fn($p) => $p->write());

        // Closure body
        $src->items->each(function ($pen) {
            $pen->write();                // resolves to Pen
        });

        // Explicit type hint takes precedence over inference
        $src->items->map(fn(Pencil $p) => $p->sketch());

        // Eloquent chunk — $orders inferred as Collection
        BlogAuthor::where('active', true)->chunk(100, function ($orders) {
            $orders->count();             // resolves to Eloquent Collection
        });

        // Eloquent whereHas — $query inferred as Builder
        BlogAuthor::whereHas('posts', function ($query) {
            $query->where('published', true); // resolves to Builder
        });

        // $this in callable param resolves to receiver, not current class
        $pipeline = new ScaffoldingPipeline();
        $pipeline->when(true, function ($pipe) {
            $pipe->send('data');          // resolves to ScaffoldingPipeline, not this demo class
        });

        // Arrow function variant
        $pipeline->tap(fn($p) => $p->through([]));
    }
}


// ═══════════════════════════════════════════════════════════════════════════
//  TRIVIAL — works in most editors, included for completeness
// ═══════════════════════════════════════════════════════════════════════════


// ── Static & Enum Completion ────────────────────────────────────────────────

class StaticEnumDemo
{
    public function demo(): void
    {
        User::$defaultRole;          // static property
        User::TYPE_ADMIN;            // class constant
        User::findByEmail('a@b.c');  // static method
        User::make('Bob');           // inherited static (Model)
        User::query();               // @mixin Builder (Model)

        Status::Active;              // backed enum case
        Status::Active->label();     // enum method
        Priority::High;              // int-backed enum
        Mode::Manual;                // unit enum
    }
}


// ── Signature Help ──────────────────────────────────────────────────────────

class SignatureHelpDemo
{
    public function demo(): void
    {
        // Place cursor inside parentheses to see parameter hints.
        // The active parameter updates as you type commas.
        $user = new User('Alice', 'alice@example.com');
        createUser('Alice', 'alice@example.com');  // standalone function
        $user->setStatus(Status::Active);          // instance method
        User::findByEmail('alice@example.com');    // static method
        new User('Bob', 'bob@example.com');        // constructor

        // Chains resolve through return types and properties:
        $user->getProfile()->setBio('Hello');                       // method return chain
        (new User('X', 'x@x.com'))->setStatus(Status::Active);     // (new ...)->method
        new User('X', 'x@x.com')->setStatus(Status::Active);     // PHP 8.4 style

        // Default values appear in parameter labels (e.g. "int $page = 1"):
        $svc = new ScaffoldingSignatureHelp();
        $svc->paginate(2, 50);

        // Per-parameter @param descriptions appear next to each parameter.
        // When the effective docblock type differs from the native PHP type
        // the description is prefixed with the effective type:
        $svc->search('php', 1, 25);
    }
}


// ── Callable Snippet Insertion ──────────────────────────────────────────────

class SnippetInsertionDemo
{
    public function demo(): Response
    {
        // Completion inserts snippets with tab-stops for required params
        $user = new User('Alice', 'alice@example.com');
        $user->setName('Bob');                    // → setName(${1:$name})
        $user->toArray();                         // → toArray()  (no params)
        $user->addRoles();                        // → addRoles() (variadic — no tab-stops)
        User::findByEmail('a@b.c');               // → findByEmail(${1:$email})
        return new Response(200);                 // → Response(${1:$statusCode})
    }
}


// ── Go-to-Definition ────────────────────────────────────────────────────────
// All jump targets are defined right after the demo so Ctrl+Click lands
// within a few lines, making it easy to verify the feature works.
//
// Member names deliberately collide with names elsewhere in the file
// (label, format, CONNECTION, $defaultRole) so a wrong-target bug
// would land on the wrong label() or CONNECTION instead of silently passing.

class GoToDefinitionDemo
{
    public function demo(): void
    {
        // Ctrl+Click on any symbol to jump to its definition
        $target = new GtdTarget();
        $target->label();                         // Ctrl+Click → GtdTarget::label() (not Pen::label)
        $target->format();                        // Ctrl+Click → GtdTarget::format() (not User::format)
        GtdTarget::FORMAT;                        // Ctrl+Click → class constant (not Renderable::format)
        GtdParent::CONNECTION;                    // Ctrl+Click → GtdParent (not Model::CONNECTION)
        GtdTarget::$defaultRole;                  // Ctrl+Click → GtdTarget (not User::$defaultRole)

        $helper = gtdHelper();
        echo $helper;                             // Ctrl+Click on $helper → jumps to assignment

        define('APP_VERSION', '1.0.0');
        echo APP_VERSION;                         // BUG: Ctrl+Click should jump to define() above
    }
}

class GtdParent { public const string CONNECTION = 'gtd'; }
class GtdTarget extends GtdParent
{
    public static string $defaultRole = 'gtd';
    public const string FORMAT = 'gtd';
    public function label(): string { return 'gtd'; }
    public function format(): string { return 'gtd'; }
}
function gtdHelper(): GtdTarget { return new GtdTarget(); }


// ── Type Hint Go-to-Definition ──────────────────────────────────────────────
// Ctrl+Click on class names in type hints, return types, catch blocks,
// and docblock annotations to jump to their definitions.
// All referenced types are defined right after the demo so the jump is short.
//
// Support classes have format()/label() methods that collide with names
// elsewhere — if GTD resolves the wrong class, you land on the wrong one.

class TypeHintGtdDemo
{
    public function demo(): void
    {
        // Catch block exception types — Ctrl+Click GtdNotFoundException or GtdAccessException
        try {
            $this->paramTypes(new GtdAlpha());
        } catch (GtdNotFoundException|GtdAccessException $e) {}
    }

    public function paramTypes(GtdAlpha $item): GtdAlpha { return $item; }                             // Ctrl+Click GtdAlpha
    public function unionTypes(GtdAlpha|GtdBeta $item): GtdAlpha|GtdBeta { return $item; }             // Ctrl+Click either
    public function intersectionTypes(GtdShape&GtdColor $item): GtdShape&GtdColor { return $item; }    // Ctrl+Click either
    public function returnType(): GtdResult { return new GtdResult(); }                                // Ctrl+Click GtdResult

    /**
     * @param list<GtdAlpha> $items                Ctrl+Click GtdAlpha
     * @return GtdResult                           Ctrl+Click GtdResult
     * @throws GtdNotFoundException                Ctrl+Click GtdNotFoundException
     */
    public function docblockTypes($items) { return $items; }

    /**
     * Callable types in docblocks. Ctrl+Click on any class name inside the
     * callable signature to jump to its definition. Hover shows the class
     * info instead of treating the whole callable as one token.
     *
     * @param \Closure(GtdAlpha): GtdResult $transform      Ctrl+Click GtdAlpha or GtdResult
     * @param callable(GtdAlpha, GtdBeta): GtdResult $merge Ctrl+Click any of the three
     * @return callable(): GtdResult                         Ctrl+Click GtdResult
     */
    public function callableDocblockTypes($transform, $merge) { return $merge; }
}

class GtdAlpha { public function label(): string { return 'alpha'; } }
class GtdBeta { public function label(): string { return 'beta'; } }
interface GtdShape { public function format(): string; }
interface GtdColor { public function format(): string; }
class GtdResult { public function label(): string { return 'ok'; } }
class GtdNotFoundException extends \RuntimeException {}
class GtdAccessException extends \RuntimeException {}


// ── Go-to-Implementation ────────────────────────────────────────────────────
// All implementors are defined right after the demo so "Go to Implementations"
// lands within a few lines.
//
// The interface method is format() — same name as Renderable::format(),
// User::format(), Ingredient::format(). A resolver bug would jump to one
// of those instead of the local implementor.

class GoToImplementationDemo
{
    // Right-click → "Go to Implementations" on GtdPrintable
    // to jump to GtdPlainPrinter and GtdHtmlPrinter below.
    // Try: Go-to-Implementation on "format" → format() in each implementor
    public function demo(GtdPrintable $printer): string
    {
        return $printer->format();
    }
}

interface GtdPrintable { public function format(): string; }
class GtdPlainPrinter implements GtdPrintable { public function format(): string { return 'plain'; } }
class GtdHtmlPrinter implements GtdPrintable { public function format(): string { return '<b>html</b>'; } }


// ── Context-Aware Class Name Filtering ──────────────────────────────────────
// Try: erase the class name after each keyword and re-trigger completion.
//
// extends Model        → classes only, non-final
//                        MUST show: User, Response, Pen (non-final classes)
//                        MUST NOT show: AdminUser (final), Model (abstract),
//                        Renderable (interface), HasTimestamps (trait), Status (enum)
//
// extends Renderable   → interfaces only (interface-extends-interface)
//                        MUST show: Renderable, GtdShape, Printable
//                        MUST NOT show: User (class), HasTimestamps (trait), Status (enum)
//
// implements Renderable → interfaces only
//                        MUST show: Renderable, GtdShape, Printable
//                        MUST NOT show: User (class), HasTimestamps (trait), Status (enum)
//
// use HasTimestamps    → traits only (inside class body)
//                        MUST show: HasTimestamps, HasSlug, JsonSerializer
//                        MUST NOT show: User (class), Renderable (interface), Status (enum)
//
// instanceof User      → classes, interfaces, enums (no traits)
//                        MUST show: User, Renderable, Status
//                        MUST NOT show: HasTimestamps (trait)
//
// new User             → concrete non-abstract classes only
//                        MUST show: User, Pen, Response
//                        MUST NOT show: Model (abstract), AdminUser (final is ok for new),
//                        Renderable (interface), HasTimestamps (trait), Status (enum)

class ClassFilteringDemo extends Model implements Renderable
{
    use HasTimestamps;
    public function test(): bool { return $this instanceof User; }
    public function format(string $template): string { return ''; }
    public function toArray(): array { return []; }
}


// ── Type Hint Completion in Definitions ─────────────────────────────────────
// Try: trigger completion when typing a type hint — PHP scalars (string,
// int, float, bool) appear alongside class names, with no constants or
// functions in the list.

function typeHintDemo(User $user, string $name): string { return $user->displayName . $name; }

function unionDemo(string|int $value, ?User $maybe): string { return $maybe . $maybe->displayName; }


// ── $_SERVER Superglobal ────────────────────────────────────────────────────

class ServerSuperglobalDemo
{
    public function demo(): void
    {
        $_SERVER[''];   // Try: key completion for REQUEST_METHOD, HTTP_HOST, etc.
    }
}


// ═══════════════════════════════════════════════════════════════════════════
//  ADVANCED — specialized features
// ═══════════════════════════════════════════════════════════════════════════


// ── Intersection Types ──────────────────────────────────────────────────────

class IntersectionDemo
{
    public function demo(Envelope&Printable $item): void
    {
        $item->print();                       // from Printable
        $item->seal();                        // from Envelope
    }
}


// ── Ternary Narrowing ──────────────────────────────────────────────────────

class TernaryNarrowingDemo
{
    public function demo(): void
    {
        $thing = pickRockOrBanana();
        $thing instanceof Rock ? $thing->crush() : $thing->peel();
    }
}


// ── Class Alias ─────────────────────────────────────────────────────────────

class ClassAliasDemo
{
    public function demo(): void
    {
        $profile = new Profile(new User('Eve', 'eve@example.com'));
        $profile->getDisplayName();               // Profile → UserProfile via `use ... as`
    }
}


// ── self::class / static::class ─────────────────────────────────────────────

class SelfClassDemo
{
    public function demo(): string
    {
        return self::class;          // resolves to SelfClassDemo
    }
}


// ── Trait insteadof / as Conflict Resolution ────────────────────────────────

class TraitConflictDemo
{
    use JsonSerializer, XmlSerializer {
        JsonSerializer::serialize insteadof XmlSerializer;
        XmlSerializer::serialize as serializeXml;
        JsonSerializer::serialize as private internalSerialize;
    }

    public function demo(): void
    {
        $this->internalSerialize();       // aliased as private
        $this->serialize();               // JsonSerializer wins via insteadof
        $this->serializeXml();            // XmlSerializer::serialize aliased
        $this->toJson();                  // non-conflicting from JsonSerializer
        $this->toXml();                   // non-conflicting from XmlSerializer
    }
}


// ── unset() Tracking ────────────────────────────────────────────────────────

class UnsetDemo
{
    public function demo(): void
    {
        $pen = new Pen('blue');
        $pen->write();                    // resolves to Pen
        unset($pen);
        // Try: $pen->  — no completions (variable was unset)

        // Re-assigning after unset restores type
        $tool = new Pen('red');
        unset($tool);
        $tool = new Marker();
        $tool->highlight();               // resolves to Marker

        // unset only affects targeted variable
        $pen2 = new Pen('green');
        $pencil = new Pencil();
        unset($pen2);
        $pencil->sketch();                // still resolves to Pencil
    }
}


// ── First-Class Callable Syntax (PHP 8.1) ───────────────────────────────────

class FirstClassCallableDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingFirstClassCallable();

        $fun = makePen(...);
        $fun()->write();                   // function reference → Closure returning Pen

        $orderFn = $src->dispatch(...);
        $orderFn()->write();              // instance method → Closure returning Pen

        $finder = Pen::make(...);
        $finder()->color();               // static method → Closure returning Pen

        $make = makePen(...);
        $pen = $make();
        $pen->color();                    // assigned result from callable invocation
    }
}


// ── Array Element Access from Assignments ───────────────────────────────────

class ArrayAccessDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingArrayAccess();

        $pens = $src->fetchAll();         // Pen[] from method return
        $pens[0]->write();                // resolves to Pen

        $gifts = (new ScaffoldingGenericContext())
            ->display();
        $gifts[0]->open();                // resolves to Gift (element of Gift[])

        $first = $pens[0];
        $first->color();                  // resolves via $first = $pens[0]
    }
}


// ── Closure / Arrow-Function Members ────────────────────────────────────────

class ClosureMembersDemo
{
    public function demo(): void
    {
        $typedClosure = function(Pen $pen): string { return $pen->write(); };
        $typedClosure->bindTo($this);     // resolves to Closure::bindTo
        $typedClosure->call($this);       // resolves to Closure::call

        $typedArrow = fn(int $posX): float => $posX * 1.5;
        $typedArrow->bindTo($this);       // resolves to Closure::bindTo

        $fun = function(): void {};
        $bound = $fun->bindTo($this);
        $bound->call($this);             // chained: $bound is still Closure
    }
}


// ── Deprecation Messages ────────────────────────────────────────────────────
// Hover over deprecated members to see the message text from @deprecated.
// Completion shows deprecated items with strikethrough styling.

class DeprecationDemo
{
    public function demo(): void
    {
        $src = new ScaffoldingDeprecation();

        // Hover: shows "🪦 **deprecated** Use sendAsync() instead."
        $src->sendLegacy();

        // Hover: shows bare "🪦 **deprecated**" (no message)
        $src->oldProcess();

        // Hover on the property: shows deprecation message
        $src->debugMode;

        // Hover on the constant: shows value and deprecation message
        ScaffoldingDeprecation::OLD_LIMIT;

        // Hover on any constant: shows its value inline (e.g. const MAX_LIMIT = 500;)
        ScaffoldingDeprecation::MAX_LIMIT;

        // ── #[Deprecated] attribute ─────────────────────────────────
        // PHPantom reads #[Deprecated] from both phpstorm-stubs
        // (\JetBrains\PhpStorm\Deprecated with reason:/since:) and
        // native PHP 8.4 (\Deprecated with message:/since:).

        // JetBrains stubs style: reason: + since:
        $src->attrDeprecatedMethod();

        // Native PHP 8.4 style: message: + since:
        $src->nativeDeprecatedMethod();

        // Bare #[Deprecated] (no arguments)
        $src->attrBareMethod();

        // Positional reason: #[Deprecated("...")]
        $src->attrPositionalMethod();

        // Attribute on property
        $src->attrProp;

        // Attribute on constant
        ScaffoldingDeprecation::ATTR_OLD;

        // Docblock @deprecated wins when both are present
        $src->bothDocAndAttr();
    }
}


// ── Hover: Origin Indicators ────────────────────────────────────────────────

class HoverOriginsDemo extends Model implements Renderable
{
    public function demo(): void
    {
        // Hover on `format` → "◆ implements Renderable"
        $this->format();

        // Hover on `toArray` → "↑ overrides Model"
        $this->toArray();

        // Hover on `getName` → no indicator (inherited, not overridden)
        $this->getName();
    }

    // Implements Renderable (Model has no format method)
    public function format(string $template): string { return ''; }

    // Overrides the abstract toArray() from Model
    public function toArray(): array { return []; }
}



// ── Diagnostic: Unknown Class ───────────────────────────────────────────────
// `MutateArrayInsertSpec` and `Cluster` below are not imported and cannot be
// resolved — they get a yellow "Class 'X' not found" warning underline.
// This diagnostic fires for any ClassReference that PHPantom cannot resolve
// through use-map, local classes, same-namespace, class_index, classmap,
// PSR-4, or stubs.  It pairs with the "Import Class" code action: press
// Ctrl+. (Cmd+. on Mac) on the warning to import the class in one step.

// ── Code Action: Import Class ───────────────────────────────────────────────
// Place cursor on `MutateArrayInsertSpec` and press Ctrl+. (or Cmd+. on Mac)
// to see "Import `Couchbase\MutateArrayInsertSpec`" in the quick-fix menu.
// Accepting inserts a `use Couchbase\MutateArrayInsertSpec;` at the top.

class ImportClassDemo
{
    public function demo(): void
    {
        // Ctrl+. on `MutateArrayInsertSpec` → offers to import
        $spec = new MutateArrayInsertSpec('path', ['value']);

        // Ctrl+. on `Cluster` → offers to import Couchbase\Cluster
        Cluster::connect('couchbase://localhost');
    }
}


// ── Code Action: Remove Unused Import ───────────────────────────────────────
// The `use ReflectionClass;` below is unused — it appears dimmed in the editor.
// Place cursor on it and press Ctrl+. → "Remove unused import 'ReflectionClass'"

use ReflectionClass;

class RemoveUnusedImportDemo
{
    public function demo(): void
    {
        // ReflectionClass is deliberately NOT used here so its import stays dimmed.
        // Ctrl+. on the dimmed `use ReflectionClass;` above → remove it.
        $x = 42;
    }
}


// ── Hover: Enum Case Listing ────────────────────────────────────────────────
// Hover on `Status` → shows all enum cases inside the code block.
// Hover on `Priority` → shows backed enum cases with their string values.

// ── Hover: Trait Method Signatures ──────────────────────────────────────────
// Hover on `HasTimestamps` in a `use` statement → shows public method signatures.
// Hover on `JsonSerializer` → shows public methods as a scannable summary.


// ═══════════════════════════════════════════════════════════════════════════
// ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
// ┃  SCAFFOLDING — Supporting definitions below this line.              ┃
// ┃  Everything below exists to support the demos above.               ┃
// ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
//
// Keep shared classes NARROW (2-4 members). The whole point of the demos
// is that a human can glance at the completion list and immediately tell
// whether the right type resolved. A 15-member class defeats that because
// the expected item could be buried on page two.
//
// If a demo needs a richer object, create a NEW class in a demo-specific
// section instead of expanding a shared one. Every member you add to a
// shared class leaks into every demo that uses it.


// ── Demo-Specific Scaffolding ───────────────────────────────────────────────

class ScaffoldingSignatureHelp
{
    /**
     * Paginate a result set.
     *
     * @param int $page Current page number.
     * @param int $limit Max items per page.
     * @return array The paginated slice of results.
     */
    public function paginate(int $page = 1, int $limit = 25): array { return []; }

    /**
     * Search for items matching a query.
     *
     * @param non-empty-string $query The search keywords.
     * @param positive-int $page Page number to return.
     * @param int $perPage Results per page.
     * @return list<array{id: int, title: string}> Matching items.
     */
    public function search(string $query, int $page = 1, int $perPage = 20): array { return []; }
}

class ScaffoldingDeprecation
{
    /**
     * @deprecated Use sendAsync() instead.
     */
    public function sendLegacy(): void {}

    /**
     * @deprecated
     */
    public function oldProcess(): void {}

    public function sendAsync(): void {}

    /**
     * @deprecated Use isDebug() instead.
     */
    public bool $debugMode = false;

    /**
     * @deprecated Use MAX_LIMIT instead.
     */
    const OLD_LIMIT = 100;

    const MAX_LIMIT = 500;

    // JetBrains stubs style
    #[\JetBrains\PhpStorm\Deprecated(reason: "Use modernMethod() instead", since: "8.1")]
    public function attrDeprecatedMethod(): void {}

    // Native PHP 8.4 style (\Deprecated)
    #[\Deprecated(message: "Use nativeModern() instead", since: "8.4")]
    public function nativeDeprecatedMethod(): void {}

    #[\Deprecated]
    public function attrBareMethod(): void {}

    #[\Deprecated("Use positionalModern() instead")]
    public function attrPositionalMethod(): void {}

    #[\JetBrains\PhpStorm\Deprecated(reason: "The property is deprecated", since: "8.4")]
    public string $attrProp = '';

    #[\Deprecated(reason: "Use NEW_SETTING instead")]
    const ATTR_OLD = 0;

    /**
     * @deprecated Docblock message wins.
     */
    #[\Deprecated(reason: "Attribute message loses")]
    public function bothDocAndAttr(): void {}
}

/**
 * @property string $gorilla
 * @method bool hyena(string $x)
 */
class Zoo extends ZooBase implements ZooContract
{
    use ZooTraitA;
    use ZooTraitB;

    public string $baboon = '';
    protected string $keeper = 'hidden';      // trip wire — must NOT appear on $zoo->
    private string $ceo = 'invisible';        // trip wire — must NOT appear on $zoo->

    public function aardvark(): void {}
    private function nocturnal(): void {}     // trip wire — must NOT appear on $zoo->

    public function __construct(
        public int $buffalo = 0,
    ) {
        parent::__construct();
    }
}

abstract class ZooBase
{
    public function __construct(
        public readonly string $cheetah = '',
    ) {}

    public function falcon(): string { return ''; }
}

trait ZooTraitA
{
    public function dingo(): void {}
}

trait ZooTraitB
{
    public function elephant(string $value): string { return $value; }
}

/**
 * @property-read string $iguana
 * @method string jaguar()
 */
interface ZooContract {}

class ScaffoldingChainingDemo
{
    public Brush $brush;
    public Canvas $canvas;

    public function __construct()
    {
        $this->brush = new Brush();
        $this->canvas = new Canvas();
    }
}

class ScaffoldingExpressionType
{
    public ?Container $backup;
    public Response $primary;

    public function __construct()
    {
        $this->backup = new Container();
        $this->primary = new Response(200);
    }
}

class ScaffoldingCollectionForeach
{
    public UserEloquentCollection $members;

    public function allMembers(): UserEloquentCollection
    {
        return new UserEloquentCollection();
    }
}

class ScaffoldingGenericContext
{
    /** @var Box<Gift> */
    public $chest;

    /** @return TypedCollection<int, Gift> */
    public function display(): TypedCollection { return new TypedCollection(); }
}

class ScaffoldingIteration
{
    /** @var list<Pen> */
    public array $batch;

    /** @return list<Pen> */
    public function allPens(): array { return []; }

    /** @return array<Pen, Pencil> */
    public function crossRef(): array { return []; }
}

class ScaffoldingArrayFunc
{
    /** @var list<Pen> */
    public array $members;

    /** @return list<Pen> */
    public function roster(): array { return []; }
}

class ScaffoldingException
{
    protected function lookup(int $id): ?array { return null; }
    protected function riskyOperation(): void {}

    /** @throws \Exception */
    protected function throwsException(): void { throw new \Exception('error'); }
}

class ScaffoldingClosureParamInference
{
    /** @var FluentCollection<int, Pen> */
    public FluentCollection $items;
}

class ScaffoldingPipeline
{
    /**
     * @param callable($this, mixed): $this $callback
     * @return $this
     */
    public function when(bool $condition, callable $callback): static { return $this; }

    /**
     * @param callable($this): void $callback
     * @return $this
     */
    public function tap(callable $callback): static { return $this; }

    public function send(mixed $data): static { return $this; }
    public function through(array $pipes): static { return $this; }
}

class ScaffoldingFirstClassCallable
{
    public function dispatch(): Pen
    {
        return new Pen();
    }
}

class ScaffoldingArrayAccess
{
    /** @return Pen[] */
    public function fetchAll(): array { return []; }
}


// ── AST Node (template bounds demo) ────────────────────────────────────────

class AstNode
{
    /** @return AstNode|null */
    public function getParent(): ?AstNode { return null; }

    /** @return AstNode[] */
    public function getChildren(): array { return []; }

    public function getType(): string { return ''; }
}

// ── ObjectMapper (method-level @template demo) ──────────────────────────────

class ObjectMapper
{
    /**
     * @template T
     * @param T $item
     * @return TypedCollection<int, T>
     */
    public function wrap(object $item): TypedCollection
    {
        /** @var TypedCollection<int, T> */
        return new TypedCollection();
    }

    /**
     * @template T
     * @param T $item
     * @return T
     */
    public function identity(mixed $item): mixed
    {
        return $item;
    }
}


// ─── Interfaces ─────────────────────────────────────────────────────────────

/**
 * @method string render()
 * @property-read string $output
 */
interface Renderable extends Stringable
{
    public function format(string $template): string;
}

// ─── Traits ─────────────────────────────────────────────────────────────────

trait JsonSerializer {
    public function serialize(): string { return '{}'; }
    public function toJson(): string { return $this->serialize(); }
}

trait XmlSerializer {
    public function serialize(): string { return '<xml/>'; }
    public function toXml(): string { return $this->serialize(); }
}

trait HasTimestamps
{
    protected ?string $createdAt = null;

    public function getCreatedAt(): ?string
    {
        return $this->createdAt;
    }

    public function setCreatedAt(string $date): static
    {
        $this->createdAt = $date;
        return $this;
    }
}

trait HasSlug
{
    public function generateSlug(string $value): string
    {
        return strtolower(str_replace(' ', '-', $value));
    }
}

/**
 * @template TFactory
 */
trait HasFactory
{
    /** @return TFactory */
    public static function factory() {}
}

/**
 * @template TKey
 * @template TValue
 */
trait Indexable
{
    /** @return TValue */
    public function get() {}

    /** @return TKey */
    public function key() {}
}

// ─── Enums ──────────────────────────────────────────────────────────────────

enum Status: string
{
    case Active = 'active';
    case Inactive = 'inactive';
    case Pending = 'pending';

    public function label(): string
    {
        return match ($this) {
            self::Active   => 'Active',
            self::Inactive => 'Inactive',
            self::Pending  => 'Pending',
        };
    }

    public function isActive(): bool
    {
        return $this === self::Active;
    }
}

enum Priority: int
{
    case Low = 1;
    case Medium = 2;
    case High = 3;
}

enum Mode
{
    case Automatic;
    case Manual;
}

// ─── Builder (@mixin target) ────────────────────────────────────────────────

class Builder
{
    /** @return static */
    public static function query(): self
    {
        return new static();
    }

    public function where(string $col, mixed $val): self
    {
        return $this;
    }
}

// ─── Abstract Base Class ────────────────────────────────────────────────────

/**
 * @property string $magicName
 * @method static static create(array $attributes)
 * @mixin Builder
 */
abstract class Model
{
    protected int $id;

    public const string CONNECTION = 'default';
    protected const int PER_PAGE = 15;

    public function __construct(
        protected string $name = '',
        public readonly string $uuid = '',
    ) {
        $this->id = rand(1, 99999);
    }

    public function getId(): int
    {
        return $this->id;
    }

    public function getName(): string
    {
        return $this->name;
    }

    /** @return static */
    public function setName(string $name): static
    {
        $this->name = $name;
        return $this;
    }

    /** @deprecated */
    public static function find(int $id): ?static
    {
        return null;
    }

    /** @return static */
    public static function make(string $name = ''): static
    {
        return new static($name, '');
    }

    abstract public function toArray(): array;

    public function __toString(): string
    {
        return $this->name;
    }
}


// ─── Concrete Classes ───────────────────────────────────────────────────────

/**
 * @property string $displayName
 * @property-read bool $isAdmin
 * @method bool hasPermission(string $permission)
 */
class User extends Model implements Renderable
{
    use HasTimestamps;
    use HasSlug;

    public string $email;
    protected Status $status;
    private array $roles = [];
    public static string $defaultRole = 'user';
    public const string TYPE_ADMIN = 'admin';
    public const string TYPE_USER = 'user';

    public function __construct(
        string $name,
        string $email,
        private readonly string $password = '',
        public int $age = 0,
    ) {
        parent::__construct($name);
        $this->email = $email;
        $this->status = Status::Active;
    }

    public function getEmail(): string
    {
        return $this->email;
    }

    public function getStatus(): Status
    {
        return $this->status;
    }

    public function setStatus(Status $status): self
    {
        $this->status = $status;
        return $this;
    }

    public function addRoles(string ...$roles): void
    {
        $this->roles = array_merge($this->roles, $roles);
    }

    public function getRoles(): array
    {
        return $this->roles;
    }

    public function getProfile(): UserProfile
    {
        return new UserProfile($this);
    }

    public function toArray(): array
    {
        return [
            'id' => $this->getId(),
            'name' => $this->getName(),
            'email' => $this->email,
            'status' => $this->status->value,
        ];
    }

    public function format(string $template): string
    {
        return str_replace('{name}', $this->getName(), $template);
    }

    public static function findByEmail(string $email): ?self
    {
        return null;
    }

    protected function hashPassword(string $raw): string
    {
        return password_hash($raw, PASSWORD_BCRYPT);
    }

    private function secretInternalMethod(): void {}
}

class UserProfile
{
    public string $bio = '';

    public function __construct(private User $user) {}

    public function getUser(): User
    {
        return $this->user;
    }

    public function setBio(string $bio): self
    {
        $this->bio = $bio;
        return $this;
    }

    public function getDisplayName(): string
    {
        return $this->user->getName() . ' (' . $this->user->getEmail() . ')';
    }
}

final class AdminUser extends User
{
    /** @var string[] */
    private array $permissions = [];

    public function __construct(string $name, string $email)
    {
        parent::__construct($name, $email);
    }

    public function toArray(): array
    {
        $base = parent::toArray();
        $base['connection'] = parent::CONNECTION;
        $base['permissions'] = $this->permissions;
        return $base;
    }

    public function grantPermission(string $permission): void
    {
        $this->permissions[] = $permission;
    }
}

class Response
{
    public function __construct(
        private string|int $statusCode,
        private string|array|null $body = null,
    ) {}

    public function getStatusCode(): string|int
    {
        return $this->statusCode;
    }

    public function getBody(): string|array|null
    {
        return $this->body;
    }

    public function isSuccess(): bool
    {
        return $this->statusCode >= 200 && $this->statusCode < 300;
    }
}

// ─── Generics (@template / @extends) ───────────────────────────────────────

/**
 * @template T
 */
class Repository
{
    /** @var T|null */
    protected $cached = null;

    /** @return T */
    public function find(int $id)
    {
        return $this->cached;
    }

    /** @return T|null */
    public function findOrNull(int $id)
    {
        return $this->cached;
    }

    /** @return T */
    public function first()
    {
        return $this->cached;
    }
}

/** @extends Repository<Pen> */
class PenRepository extends Repository {}

class CachingPenRepository extends PenRepository
{
    public function clearCache(): void {}
}

/**
 * @template TKey of array-key
 * @template-covariant TValue
 */
class TypedCollection
{
    /** @var array<TKey, TValue> */
    protected array $items = [];

    /** @return TValue */
    public function first() { return reset($this->items); }

    /** @return ?TValue */
    public function last() { return end($this->items) ?: null; }

    /** @return static */
    public function filter(callable $fn): static { return $this; }

    /** @return int */
    public function count(): int { return count($this->items); }

    /** @return array<TKey, TValue> */
    public function all(): array { return $this->items; }
}

/** @extends TypedCollection<int, Pen> */
class PenCollection extends TypedCollection
{
    public function thickOnly(): self
    {
        return $this;
    }
}

/** @phpstan-extends TypedCollection<string, Response> */
class ResponseCollection extends TypedCollection {}

// ─── Container (conditional return types) ───────────────────────────────────

class Container
{
    /** @var array<string, object> */
    private array $bindings = [];

    /**
     * @template TClass
     * @param string|null $abstract
     * @return ($abstract is class-string<TClass> ? TClass : mixed)
     */
    public function make(?string $abstract = null): mixed
    {
        if ($abstract === null) {
            return $this;
        }
        return $this->bindings[$abstract] ?? new Exception();
    }

    public function bind(string $abstract, object $obj): void
    {
        $this->bindings[$abstract] = $obj;
    }

    public function getStatus(): int
    {
        return 200;
    }
}

// ─── Method-Level @template Classes ─────────────────────────────────────────

class ServiceLocator
{
    /**
     * @template T
     * @param class-string<T> $id
     * @return T
     */
    public function get(string $id): object
    {
        return new \stdClass();
    }
}

class Factory
{
    /**
     * @template T
     * @param class-string<T> $class
     * @return T
     */
    public static function create(string $class): object
    {
        return new $class();
    }
}

// ─── Generic Wrapper ────────────────────────────────────────────────────────

/**
 * @template T
 */
class Box
{
    /** @var T */
    public $value;

    /** @return T */
    public function unwrap() { return $this->value; }
}

class Gift
{
    public function open(): string { return 'surprise!'; }
    public function getTag(): string { return 'birthday'; }
}

// ─── Narrowing Demo Support Classes ─────────────────────────────────────────

class Rock
{
    public function crush(): string { return 'smash!'; }
    public function weigh(): float { return 5.0; }
}

class Banana
{
    public function peel(): string { return 'yum!'; }
    public function weigh(): float { return 0.2; }
}

// ─── Ambiguous Variable Support Classes ─────────────────────────────────────

class Lamp
{
    public function dim(): void {}
    public function turnOff(): void {}
}

class Faucet
{
    public function drip(): void {}
    public function turnOff(): void {}
}

// ─── Intersection Demo Support Classes ──────────────────────────────────────

interface Printable
{
    public function print(): void;
}

class Envelope
{
    public function seal(): void {}
}

// ─── Shared Narrow Classes ──────────────────────────────────────────────────
// These are small, purpose-built classes for demos. Keep them narrow (2-4
// members each). If a demo needs a richer object, create a new class in a
// demo-specific section below instead of expanding these.

class Pen
{
    public function __construct(public string $ink = 'black') {}
    public function write(): string { return ''; }
    public function color(): string { return $this->ink; }
    public function label(): string { return 'pen'; }
    /** @return static */
    public function rename(string $name): static { return $this; }
    /** @return static */
    public static function make(): static { return new static(); }
    private function refill(): void {}            // trip wire — must NOT appear on external $pen->
}

class Pencil
{
    public function sketch(): string { return ''; }
    public function sharpen(): void {}
    public function label(): string { return 'pencil'; }
}

class Marker extends Pen
{
    public function highlight(): void {}
}

// ─── Chaining Demo Support Classes ──────────────────────────────────────────

class Brush
{
    public function setSize(string $size): static { return $this; }
    public function setStyle(string $style): static { return $this; }
    public function stroke(): string { return ''; }
    public function getCanvas(): Canvas { return new Canvas(); }
    protected function calibrate(): void {}       // trip wire — must NOT appear on $studio->brush->

    /** @deprecated */
    public static function find(int $id): ?static { return null; }
}

class Canvas
{
    public Easel $easel;

    public function __construct() { $this->easel = new Easel(); }
    public function getBrush(): Brush { return new Brush(); }
    public function title(): string { return ''; }
}

class Easel
{
    public string $material = 'wood';
    public function height(): string { return '150cm'; }
}

// ─── Expression Type Support Classes ────────────────────────────────────────

class ElasticProductReviewIndexService
{
    public function index(array $markets = []): void {}
    public function reindex(): void {}
}

class ElasticBrandIndexService
{
    public function index(array $markets = []): void {}
    public function bulkDelete(array $ids): void {}
}

// ─── Param Override Support Classes ─────────────────────────────────────────

class Ingredient
{
    public string $name = '';
    public float $quantity = 0.0;

    public function format(): string
    {
        return "{$this->quantity}x {$this->name}";
    }
}

class Recipe
{
    /**
     * @param list<Ingredient> $ingredients
     */
    public function __construct(
        public array $ingredients = [],
        public string $title = '',
    ) {}
}

// ─── Trait Generic Support Classes ──────────────────────────────────────────

class UserFactory
{
    public function create(): User { return new User('', ''); }
    public function count(int $n): static { return $this; }
    public function state(array $state): static { return $this; }
    public function make(): User { return new User('', ''); }
}

/** @use HasFactory<UserFactory> */
class Product
{
    use HasFactory;

    public function getPrice(): float { return 0.0; }
}

/** @use Indexable<int, Pen> */
class PenIndex
{
    use Indexable;
}

// ─── Exception Classes ──────────────────────────────────────────────────────

class NotFoundException extends \RuntimeException {}
class ValidationException extends \RuntimeException {}
class AuthorizationException extends \RuntimeException {}

// ─── Standalone Functions ───────────────────────────────────────────────────

/**
 * @template TClass
 * @param string|null $abstract
 * @return ($abstract is class-string<TClass> ? TClass : Container)
 */
function app(?string $abstract = null): mixed
{
    static $container = null;
    if ($container === null) {
        $container = new Container();
    }
    return $abstract !== null ? $container->make($abstract) : $container;
}

function createUser(string $name, string $email): User
{
    return new User($name, $email);
}

function makePen(): Pen
{
    return new Pen();
}

function pickPenOrPencil(): Pen|Pencil
{
    return new Pen();
}

function getUnknownValue(): mixed
{
    return new AdminUser('', '');
}

/**
 * @template T
 * @param class-string<T> $class The class name
 * @return T
 */
function resolve(string $class): object
{
    return new $class();
}

/**
 * @return array{logger: Pen, debug: bool}
 */
function getAppConfig(): array { return []; }

function pickRockOrBanana(): Rock|Banana
{
    return new Rock();
}

/** @phpstan-assert Rock $value */
function assertRock(mixed $value): void
{
    if (!$value instanceof Rock) {
        throw new \InvalidArgumentException('Expected Rock');
    }
}

/** @phpstan-assert-if-true Rock $value */
function isRock(mixed $value): bool
{
    return $value instanceof Rock;
}

/** @phpstan-assert-if-false Rock $value */
function isNotRock(mixed $value): bool
{
    return !$value instanceof Rock;
}

// ─── Multi-line @return & Broken Docblock Recovery ──────────────────────────

/**
 * @template TKey of array-key
 * @template TValue
 */
class FluentCollection
{
    /**
     * @template TGroupKey of array-key
     *
     * @param  (callable(TValue, TKey): TGroupKey)|array|string  $groupBy
     * @param  bool  $preserveKeys
     * @return static<
     *  ($groupBy is (array|string)
     *      ? array-key
     *      : TGroupKey),
     *  static<($preserveKeys is true ? TKey : int), TValue>
     * >
     */
    public function groupBy($groupBy, $preserveKeys = false)
    {
    }

    /**
     * @template TMapValue
     *
     * @param  callable(TValue, TKey): TMapValue  $callback
     * @return static<TKey, TMapValue>
     */
    public function map(callable $callback)
    {
    }

    /**
     * @param  callable(TValue, TKey): void  $callback
     * @return static<TKey, TValue>
     */
    public function each(callable $callback)
    {
    }

    /**
     * @return array<
     *   string,
     *   FluentCollection<int, TValue>
     * >
     */
    public function toGroupedArray()
    {
    }

    /**
     * @return static<TKey, TValue>
     */
    public function values()
    {
    }
}

/** @return FluentCollection */
function collect(mixed $value = []): FluentCollection
{
    return new FluentCollection();
}

class BrokenDocRecovery
{
    /**
     * Broken multi-line @return — base `static` is recovered.
     * @return static<
     */
    public function broken(): static
    {
        return $this;
    }

    public function working(): string
    {
        return 'hello';
    }
}

// ─── Foreach over Generic Collection Classes ────────────────────────────────

/**
 * @template TKey of array-key
 * @template-covariant TValue
 * @implements \IteratorAggregate<TKey, TValue>
 */
class BaseCollection implements \IteratorAggregate
{
    /** @return \ArrayIterator<TKey, TValue> */
    public function getIterator(): \ArrayIterator { return new \ArrayIterator([]); }
}

/**
 * @template TKey of array-key
 * @template TModel of Model
 * @extends BaseCollection<TKey, TModel>
 */
class EloquentCollection extends BaseCollection {}

/**
 * @extends EloquentCollection<int, User>
 */
final class UserEloquentCollection extends EloquentCollection {}

// ── Laravel Relationship Demo Models ────────────────────────────────────────

// ── Bakery — Alphabetical Eloquent property demo model ──────────────────────
// One virtual member per letter (a–v), each from a different source.
// Trigger `$bakery->` in EloquentPropertyDemo and verify a–v in order.

class Bakery extends \Illuminate\Database\Eloquent\Model
{
    protected $fillable = ['flour'];

    protected $guarded = ['kitchen_id'];

    protected $hidden = ['oven_code'];

    protected $visible = ['rye_blend'];

    protected $casts = [
        'apricot'    => 'boolean',
        'dough_temp' => 'float',
        'icing'      => FrostingCast::class,
        'jam_flavor' => JamFlavor::class,
        'notes'      => 'array',
        'proved_at'  => 'datetime',
    ];

    protected function casts(): array
    {
        return [
            'quality' => 'float',
        ];
    }

    protected $attributes = [
        'croissant'   => 'plain',
        'egg_count'   => 0,
        'gluten_free' => false,
    ];

    /** @return \Illuminate\Database\Eloquent\Relations\HasMany<Loaf, $this> */
    public function baguettes(): mixed { return $this->hasMany(Loaf::class); }

    /** @return \Illuminate\Database\Eloquent\Relations\HasOne<Baker, $this> */
    public function headBaker(): mixed { return $this->hasOne(Baker::class); }

    /** @return \Illuminate\Database\Eloquent\Relations\BelongsToMany<BakeryRecipe, $this> */
    public function masterRecipe(): mixed { return $this->belongsToMany(BakeryRecipe::class); }

    public function vendor() { return $this->morphTo(); }

    public function scopeTopping(\Illuminate\Database\Eloquent\Builder $query, string $type): void
    {
        $query->where('topping', $type);
    }

    public function scopeUnbaked(\Illuminate\Database\Eloquent\Builder $query): void
    {
        $query->where('baked', false);
    }

    #[\Illuminate\Database\Eloquent\Attributes\Scope]
    protected function fresh(\Illuminate\Database\Eloquent\Builder $query): void
    {
        $query->where('fresh', true);
    }

    public function getLoafNameAttribute(): string { return ''; }

    /** @return \Illuminate\Database\Eloquent\Casts\Attribute<string> */
    protected function sprinkle(): \Illuminate\Database\Eloquent\Casts\Attribute
    {
        return new \Illuminate\Database\Eloquent\Casts\Attribute();
    }
}

class Loaf extends \Illuminate\Database\Eloquent\Model
{
    public function getWeight(): int { return 0; }
}

class Baker extends \Illuminate\Database\Eloquent\Model
{
    public function getName(): string { return ''; }
}

class BakeryRecipe extends \Illuminate\Database\Eloquent\Model
{
    public function getTitle(): string { return ''; }
}

enum JamFlavor: string
{
    case Strawberry = 'strawberry';
    case Raspberry = 'raspberry';
    case Blueberry = 'blueberry';
}

// ── BlogAuthor — used by EloquentQueryDemo and ClosureParamInferenceDemo ────

class BlogAuthor extends \Illuminate\Database\Eloquent\Model
{
    protected $fillable = ['name', 'email', 'genre'];

    protected $guarded = ['id'];

    protected $hidden = ['password'];

    /** @return \Illuminate\Database\Eloquent\Relations\HasMany<BlogPost, $this> */
    public function posts(): mixed { return $this->hasMany(BlogPost::class); }

    /** @return \Illuminate\Database\Eloquent\Relations\HasOne<AuthorProfile, $this> */
    public function profile(): mixed { return $this->hasOne(AuthorProfile::class); }

    public function scopeActive(\Illuminate\Database\Eloquent\Builder $query): void
    {
        $query->where('active', true);
    }

    public function scopeOfGenre(\Illuminate\Database\Eloquent\Builder $query, string $genre): void
    {
        $query->where('genre', $genre);
    }
}

class BlogPost extends \Illuminate\Database\Eloquent\Model
{
    public function getTitle(): string { return ''; }
    public function getSlug(): string { return ''; }
}

class AuthorProfile extends \Illuminate\Database\Eloquent\Model
{
    public function getBio(): string { return ''; }
    public function getAvatar(): string { return ''; }
}

class BlogTag extends \Illuminate\Database\Eloquent\Model
{
    public function getLabel(): string { return ''; }
}

// ── Custom Eloquent Collection Demo Models ──────────────────────────────────

/**
 * @template TKey of array-key
 * @template TModel
 * @extends \Illuminate\Database\Eloquent\Collection<TKey, TModel>
 */
class ReviewCollection extends \Illuminate\Database\Eloquent\Collection
{
    /** @return array<TKey, TModel> */
    public function topRated(): array { return []; }

    /** @return float */
    public function averageRating(): float { return 0.0; }
}

#[\Illuminate\Database\Eloquent\Attributes\CollectedBy(ReviewCollection::class)]
class Review extends \Illuminate\Database\Eloquent\Model
{
    public function getTitle(): string { return ''; }
    public function getRating(): int { return 0; }

    /** @return \Illuminate\Database\Eloquent\Relations\HasMany<Review, $this> */
    public function replies(): mixed { return $this->hasMany(Review::class); }
}

class Frosting
{
    public function __construct(private string $flavor = '') {}
    public function getFlavor(): string { return $this->flavor; }
    public function isSweet(): bool { return $this->flavor !== ''; }
    public function __toString(): string { return $this->flavor; }
}

class FrostingCast
{
    public function get($model, string $key, mixed $value, array $attributes): ?Frosting
    {
        return new Frosting((string) $value);
    }
}

enum OrderStatus: string
{
    case Pending = 'pending';
    case Processing = 'processing';
    case Completed = 'completed';
    case Cancelled = 'cancelled';

    public function label(): string { return $this->value; }
    public function isPending(): bool { return $this === self::Pending; }
}

} // end namespace Demo

// ── Illuminate Stubs ────────────────────────────────────────────────────────
// Minimal stubs matching real Laravel classes so that the Eloquent demo models
// above resolve Builder methods, relationship properties, and scope forwarding
// without requiring a real Laravel installation.

namespace Illuminate\Database\Eloquent {

    abstract class Model {
        /** @return \Illuminate\Database\Eloquent\Builder<static> */
        public static function query() {}
    }

    /**
     * @template TModel of \Illuminate\Database\Eloquent\Model
     *
     * @mixin \Illuminate\Database\Query\Builder
     */
    class Builder implements \Illuminate\Contracts\Database\Eloquent\Builder {
        /** @use \Illuminate\Database\Concerns\BuildsQueries<TModel> */
        use \Illuminate\Database\Concerns\BuildsQueries;

        /**
         * @param  (\Closure(static): mixed)|string|array  $column
         * @return $this
         */
        public function where($column, $operator = null, $value = null, $boolean = 'and') {}

        /** @return \Illuminate\Database\Eloquent\Collection<int, TModel> */
        public function get($columns = ['*']) { return new Collection(); }

        /**
         * @param  string  $relation
         * @param  (\Closure(\Illuminate\Database\Eloquent\Builder<TModel>): mixed)|null  $callback
         * @return static
         */
        public function whereHas(string $relation, ?\Closure $callback = null): static { return $this; }

        /**
         * @param  array<array-key, array|(\Closure(\Illuminate\Database\Eloquent\Relations\Relation): mixed)|string>|string  $relations
         * @param  (\Closure(\Illuminate\Database\Eloquent\Relations\Relation): mixed)|string|null  $callback
         * @return static
         */
        public function with($relations, $callback = null): static { return $this; }
    }

    /**
     * @template TKey of array-key
     * @template TModel of \Illuminate\Database\Eloquent\Model
     */
    class Collection {
        /** @return TModel|null */
        public function first(): mixed { return null; }
        public function count(): int { return 0; }
    }
}

namespace Illuminate\Database\Eloquent\Relations {
    /**
     * @template TRelated of \Illuminate\Database\Eloquent\Model
     * @template TDeclaringModel of \Illuminate\Database\Eloquent\Model
     * @template TResult
     */
    class Relation {
        /** @return static */
        public function where(string $column, $operator = null, $value = null): static { return $this; }
        /** @return static */
        public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
    }
    class HasMany {}
    class HasOne {}
    class BelongsTo {}
    class BelongsToMany {}
    class MorphOne {}
    class MorphMany {}
    class MorphTo {}
    class MorphToMany {}
    class HasManyThrough {}
}

namespace Illuminate\Database\Eloquent\Attributes {
    class CollectedBy {}
    class Scope {}
}

namespace Illuminate\Database\Eloquent\Casts {
    class Attribute {}
}

namespace Illuminate\Database\Eloquent {
    /** @template TCollection */
    trait HasCollection {}
}

namespace Illuminate\Database\Concerns {

    /**
     * @template TValue
     */
    trait BuildsQueries {
        /** @return TValue|null */
        public function first($columns = ['*']) { return null; }

        /**
         * @param  callable(\Illuminate\Support\Collection<int, TValue>, int): mixed  $callback
         * @return bool
         */
        public function chunk(int $count, callable $callback): bool { return true; }
    }
}

namespace Illuminate\Database\Query {

    class Builder {
        /**
         * @param  string  $column
         * @return $this
         */
        public function whereIn($column, $values, $boolean = 'and', $not = false) { return $this; }

        /** @return $this */
        public function groupBy(...$groups) { return $this; }

        /** @return $this */
        public function orderBy($column, $direction = 'asc') { return $this; }

        /** @return $this */
        public function limit($value) { return $this; }

        /**
         * @return \Illuminate\Support\Collection<int, \stdClass>
         */
        public function get($columns = ['*']) {}
    }
}

namespace Illuminate\Support {

    /**
     * @template TKey of array-key
     * @template TValue
     */
    class Collection {
        /** @return int */
        public function count(): int { return 0; }
        /** @return TValue|null */
        public function first(): mixed { return null; }
        /** @return array<TKey, TValue> */
        public function all(): array { return []; }
        /**
         * @param callable(TValue, TKey): mixed $callback
         * @return static
         */
        public function each(callable $callback): static { return $this; }
    }
}

namespace Illuminate\Contracts\Database\Eloquent {
    /**
     * @mixin \Illuminate\Database\Eloquent\Builder
     */
    interface Builder {}
}

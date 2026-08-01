use super::*;
use crate::atom::atom;
use crate::test_fixtures::{make_class, no_loader};
use crate::types::ClassInfo;
use std::sync::Arc;

// ── is_eloquent_model ───────────────────────────────────────────────

#[test]
fn recognises_fqn() {
    assert!(is_eloquent_model("Illuminate\\Database\\Eloquent\\Model"));
}

#[test]
fn rejects_unrelated_class() {
    assert!(!is_eloquent_model("App\\Models\\User"));
}

// ── extends_eloquent_model ──────────────────────────────────────────

#[test]
fn direct_child_of_model() {
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some(atom("Illuminate\\Database\\Eloquent\\Model"));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    assert!(extends_eloquent_model(&user, &loader));
}

#[test]
fn indirect_child_of_model() {
    let mut user = make_class("App\\Models\\Admin");
    user.parent_class = Some(atom("App\\Models\\User"));

    let mut base = make_class("App\\Models\\User");
    base.parent_class = Some(atom("Illuminate\\Database\\Eloquent\\Model"));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        match name {
            "App\\Models\\User" => Some(Arc::new(base.clone())),
            "Illuminate\\Database\\Eloquent\\Model" => Some(Arc::new(model.clone())),
            _ => None,
        }
    };

    assert!(extends_eloquent_model(&user, &loader));
}

#[test]
fn not_a_model() {
    let service = make_class("App\\Services\\UserService");
    assert!(!extends_eloquent_model(&service, &no_loader));
}

// ── camel_to_snake ──────────────────────────────────────────────

#[test]
fn camel_to_snake_simple() {
    assert_eq!(camel_to_snake("FullName"), "full_name");
}

#[test]
fn camel_to_snake_single_word() {
    assert_eq!(camel_to_snake("Name"), "name");
}

#[test]
fn camel_to_snake_already_lower() {
    assert_eq!(camel_to_snake("name"), "name");
}

#[test]
fn camel_to_snake_camel_case() {
    assert_eq!(camel_to_snake("firstName"), "first_name");
}

#[test]
fn camel_to_snake_multiple_words() {
    assert_eq!(camel_to_snake("isAdminUser"), "is_admin_user");
}

#[test]
fn camel_to_snake_with_digit() {
    assert_eq!(camel_to_snake("item2Name"), "item2_name");
}

#[test]
fn camel_to_snake_acronym() {
    assert_eq!(camel_to_snake("URLName"), "url_name");
}

// ── legacy_accessor_method_name ─────────────────────────────────

#[test]
fn legacy_accessor_prop_name_simple() {
    assert_eq!(legacy_accessor_method_name("name"), "getNameAttribute");
}

#[test]
fn legacy_accessor_prop_name_multi_word() {
    assert_eq!(
        legacy_accessor_method_name("display_name"),
        "getDisplayNameAttribute"
    );
}

#[test]
fn legacy_accessor_prop_name_three_words() {
    assert_eq!(
        legacy_accessor_method_name("full_legal_name"),
        "getFullLegalNameAttribute"
    );
}

// ── walk_all_php_expressions ─────────────────────────────────────────────────

fn collect_strings(php: &str) -> Vec<String> {
    let mut found = Vec::new();
    walk_all_php_expressions(php, &mut |expr| {
        if let Some((s, _, _)) = extract_string_literal(expr, php) {
            found.push(s.to_string());
        }
        ControlFlow::Continue(())
    });
    found
}

fn has(php: &str, needle: &str) -> bool {
    collect_strings(php).iter().any(|s| s == needle)
}

// ── Statement branches ───────────────────────────────────────────────────────

#[test]
fn walker_return_stmt() {
    assert!(has("<?php return 'ret';", "ret"));
}

#[test]
fn walker_echo_stmt() {
    let php = "<?php echo 'ea', 'eb';";
    assert!(has(php, "ea"));
    assert!(has(php, "eb"));
}

#[test]
fn walker_namespace_stmt() {
    assert!(has("<?php namespace Foo; return 'ns';", "ns"));
}

#[test]
fn walker_block_stmt() {
    assert!(has("<?php { return 'blk'; }", "blk"));
}

#[test]
fn walker_if_stmt() {
    let php =
        "<?php if (true) { return 'th'; } elseif (false) { return 'ei'; } else { return 'el'; }";
    assert!(has(php, "th"));
    assert!(has(php, "ei"));
    assert!(has(php, "el"));
}

#[test]
fn walker_while_stmt() {
    assert!(has("<?php while (true) { return 'wb'; }", "wb"));
}

#[test]
fn walker_do_while_stmt() {
    assert!(has("<?php do { return 'dw'; } while (false);", "dw"));
}

#[test]
fn walker_for_stmt() {
    let php = "<?php for (foo('fi'); foo('fc'); foo('fu')) { foo('fb'); }";
    assert!(has(php, "fi"));
    assert!(has(php, "fc"));
    assert!(has(php, "fu"));
    assert!(has(php, "fb"));
}

#[test]
fn walker_foreach_stmt() {
    let php = "<?php foreach (['item'] as $v) { return 'fv'; }";
    assert!(has(php, "item"));
    assert!(has(php, "fv"));
}

#[test]
fn walker_try_catch_finally() {
    let php = "<?php try { return 'tv'; } catch (\\Exception $e) { return 'cv'; } finally { return 'fv'; }";
    assert!(has(php, "tv"));
    assert!(has(php, "cv"));
    assert!(has(php, "fv"));
}

#[test]
fn walker_switch_stmt() {
    let php = "<?php switch ('sw') { case 'cs': return 'rv'; default: return 'dv'; }";
    assert!(has(php, "sw"));
    assert!(has(php, "cs"));
    assert!(has(php, "rv"));
    assert!(has(php, "dv"));
}

#[test]
fn walker_function_body() {
    assert!(has("<?php function foo() { return 'fn'; }", "fn"));
}

#[test]
fn walker_class_method_body() {
    assert!(has("<?php class C { function m() { return 'cm'; } }", "cm"));
}

#[test]
fn walker_class_property() {
    assert!(has("<?php class C { public $x = 'pv'; }", "pv"));
}

#[test]
fn walker_class_constant() {
    assert!(has("<?php class C { const K = 'ck'; }", "ck"));
}

#[test]
fn walker_interface_constant() {
    assert!(has("<?php interface I { const K = 'ik'; }", "ik"));
}

#[test]
fn walker_trait_method_body() {
    assert!(has("<?php trait T { function m() { return 'tm'; } }", "tm"));
}

#[test]
fn walker_enum_backed_case() {
    assert!(has("<?php enum S: string { case A = 'ev'; }", "ev"));
}

#[test]
fn walker_enum_constant() {
    assert!(has("<?php enum S { const K = 'ec'; }", "ec"));
}

#[test]
fn walker_static_var() {
    assert!(has("<?php function f() { static $x = 'sv'; }", "sv"));
}

#[test]
fn walker_unset_stmt() {
    // unset args are variable expressions; verify the walker reaches code after it
    assert!(has("<?php unset($x); return 'au';", "au"));
}

// ── Expression branches ──────────────────────────────────────────────────────

#[test]
fn walker_method_call() {
    assert!(has("<?php $o->m('ma');", "ma"));
}

#[test]
fn walker_null_safe_method_call() {
    assert!(has("<?php $o?->m('na');", "na"));
}

#[test]
fn walker_binary_expr() {
    let php = "<?php return 'lhs' . 'rhs';";
    assert!(has(php, "lhs"));
    assert!(has(php, "rhs"));
}

#[test]
fn walker_unary_prefix() {
    assert!(has("<?php $x = !true; return 'up';", "up"));
}

#[test]
fn walker_unary_postfix() {
    assert!(has("<?php $i = 0; $i++; return 'upo';", "upo"));
}

#[test]
fn walker_parenthesized() {
    assert!(has("<?php return ('pv');", "pv"));
}

#[test]
fn walker_assignment() {
    assert!(has("<?php $x = 'av';", "av"));
}

#[test]
fn walker_conditional_ternary() {
    let php = "<?php return true ? 'th' : 'el';";
    assert!(has(php, "th"));
    assert!(has(php, "el"));
}

#[test]
fn walker_conditional_elvis() {
    let php = "<?php return 'cv' ?: 'ev';";
    assert!(has(php, "cv"));
    assert!(has(php, "ev"));
}

#[test]
fn walker_array_literal() {
    let php = "<?php return ['v1', 'k' => 'v2'];";
    assert!(has(php, "v1"));
    assert!(has(php, "k"));
    assert!(has(php, "v2"));
}

#[test]
fn walker_legacy_array() {
    assert!(has("<?php return array('la1', 'la2');", "la1"));
}

#[test]
fn walker_variadic_array_element() {
    assert!(has("<?php return [...$a, 'vae'];", "vae"));
}

#[test]
fn walker_array_access() {
    assert!(has("<?php return $a['ki'];", "ki"));
}

#[test]
fn walker_closure_body() {
    assert!(has("<?php $f = function() { return 'clv'; };", "clv"));
}

#[test]
fn walker_arrow_function() {
    assert!(has("<?php $f = fn() => 'afv';", "afv"));
}

#[test]
fn walker_match_expr() {
    let php = "<?php return match ('ms') { 'ma' => 'mv', default => 'md' };";
    assert!(has(php, "ms"));
    assert!(has(php, "ma"));
    assert!(has(php, "mv"));
    assert!(has(php, "md"));
}

#[test]
fn walker_throw_expr() {
    assert!(has("<?php throw new \\Exception('te');", "te"));
}

#[test]
fn walker_yield_value() {
    assert!(has("<?php function g() { yield 'yv'; }", "yv"));
}

#[test]
fn walker_yield_pair() {
    let php = "<?php function g() { yield 'yk' => 'yp'; }";
    assert!(has(php, "yk"));
    assert!(has(php, "yp"));
}

#[test]
fn walker_yield_from() {
    assert!(has("<?php function g() { yield from ['yf']; }", "yf"));
}

#[test]
fn walker_clone() {
    assert!(has("<?php $b = clone $a; return 'after';", "after"));
}

#[test]
fn walker_instantiation_args() {
    assert!(has("<?php new Foo('ca');", "ca"));
}

// ── ControlFlow early exit ───────────────────────────────────────────────────

#[test]
fn walker_early_exit_stops_after_break() {
    let php = "<?php foo('a'); foo('b'); foo('c');";
    let mut visited: Vec<String> = Vec::new();
    walk_all_php_expressions(php, &mut |expr| {
        if let Some((s, _, _)) = extract_string_literal(expr, php) {
            visited.push(s.to_string());
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    });
    assert_eq!(visited, vec!["a".to_string()]);
}

// ── Singularizer ─────────────────────────────────────────────────────────────

/// Every pair below was produced by running `Str::singular()` against the
/// framework in `examples/laravel`, so this pins the singularizer to the
/// real inflector rather than to what our own rules happen to do.  Laravel
/// derives a resource route's wildcard with `Str::singular()`, so a
/// disagreement here is a parameter name the editor offers that the
/// application does not have.
///
/// Some of these read oddly as English (`ties` → `ty`, `curves` → `curve`
/// only because it is in the irregular table).  That is Doctrine's
/// behaviour, and matching Laravel is what makes the feature correct.
#[test]
fn singularizes_exactly_as_laravel_does() {
    const CASES: &[(&str, &str)] = &[
        ("photos", "photo"),
        ("users", "user"),
        ("posts", "post"),
        ("comments", "comment"),
        ("categories", "category"),
        ("companies", "company"),
        ("cities", "city"),
        ("countries", "country"),
        ("tags", "tag"),
        ("orders", "order"),
        ("products", "product"),
        ("invoices", "invoice"),
        ("payments", "payment"),
        ("articles", "article"),
        ("tickets", "ticket"),
        ("reviews", "review"),
        ("replies", "reply"),
        ("entries", "entry"),
        ("libraries", "library"),
        ("roles", "role"),
        ("files", "file"),
        ("profiles", "profile"),
        ("teams", "team"),
        ("projects", "project"),
        ("settings", "setting"),
        ("menus", "menu"),
        ("addresses", "address"),
        ("boxes", "box"),
        ("taxes", "tax"),
        ("classes", "class"),
        ("matches", "match"),
        ("branches", "branch"),
        ("statuses", "status"),
        ("media", "medium"),
        ("series", "series"),
        ("news", "news"),
        ("bakeries", "bakery"),
        ("ovens", "oven"),
        ("blog-posts", "blog-post"),
        ("cookies", "cookie"),
        ("leaves", "leaf"),
        ("bonuses", "bonus"),
        ("viruses", "virus"),
        ("heroes", "hero"),
        ("quizzes", "quiz"),
        ("matrices", "matrix"),
        ("indices", "index"),
        ("vertices", "vertex"),
        ("oxen", "ox"),
        ("aliases", "alias"),
        ("potatoes", "potato"),
        ("tomatoes", "tomato"),
        ("volcanoes", "volcano"),
        ("echoes", "echo"),
        ("buffaloes", "buffalo"),
        ("fungi", "fungus"),
        ("nuclei", "nucleus"),
        ("syllabi", "syllabus"),
        ("cacti", "cactus"),
        ("radii", "radius"),
        ("faxes", "fax"),
        ("waxes", "wax"),
        ("analyses", "analysis"),
        ("crises", "crisis"),
        ("theses", "thesis"),
        ("testes", "testis"),
        ("diagnoses", "diagnosis"),
        ("parentheses", "parenthesis"),
        ("synopses", "synopsis"),
        ("bases", "basis"),
        ("prognoses", "prognosis"),
        ("shoes", "shoe"),
        ("slaves", "slave"),
        ("houses", "house"),
        ("buses", "bus"),
        ("campuses", "campus"),
        ("focuses", "focus"),
        ("octopuses", "octopus"),
        ("mice", "mouse"),
        ("lice", "louse"),
        ("dishes", "dish"),
        ("watches", "watch"),
        ("movies", "movie"),
        ("ties", "ty"),
        ("dies", "dy"),
        ("lies", "ly"),
        ("pies", "py"),
        ("soliloquies", "soliloquy"),
        ("nurseries", "nursery"),
        ("brownies", "brownie"),
        ("zombies", "zombie"),
        ("genies", "genie"),
        ("wolves", "wolf"),
        ("shelves", "shelf"),
        ("selves", "self"),
        ("halves", "half"),
        ("elves", "elf"),
        ("knives", "knife"),
        ("wives", "wife"),
        ("lives", "life"),
        ("thieves", "thief"),
        ("loaves", "loaf"),
        ("olives", "olive"),
        ("natives", "native"),
        ("drives", "drive"),
        ("hives", "hive"),
        ("curves", "curve"),
        ("valves", "valve"),
        ("waves", "wave"),
        ("caves", "cave"),
        ("graves", "grave"),
        ("moves", "move"),
        ("saves", "save"),
        ("sieves", "sieve"),
        ("safes", "safe"),
        ("criteria", "criterion"),
        ("memoranda", "memorandum"),
        ("curricula", "curriculum"),
        ("stadiums", "stadium"),
        ("people", "person"),
        ("children", "child"),
        ("men", "man"),
        ("women", "woman"),
        ("feet", "foot"),
        ("teeth", "tooth"),
        ("geese", "goose"),
        ("data", "data"),
        ("axes", "axe"),
        ("dice", "die"),
        ("monies", "money"),
        ("gases", "gas"),
        ("lenses", "lens"),
        ("irises", "iris"),
        ("atlases", "atlas"),
        ("canvases", "canvas"),
        ("corpuses", "corpus"),
        ("penises", "penis"),
        ("sexes", "sex"),
        ("hoaxes", "hoax"),
        ("niches", "niche"),
        ("caches", "cache"),
        ("cafes", "cafe"),
        ("demos", "demo"),
        ("epochs", "epoch"),
        ("humans", "human"),
        ("cows", "cow"),
        ("brothers", "brother"),
        ("foes", "foe"),
        ("turfs", "turf"),
        ("hoofs", "hoof"),
        ("beefs", "beef"),
        ("trilbys", "trilby"),
        ("plateaux", "plateau"),
        ("chateaux", "chateau"),
        ("niveaux", "niveau"),
        ("passersby", "passerby"),
        ("runners-up", "runner-up"),
        ("sons-in-law", "son-in-law"),
        ("graffiti", "graffito"),
        ("mythoi", "mythos"),
        ("numina", "numen"),
        ("genera", "genus"),
        ("larvae", "larva"),
        ("algae", "alga"),
        ("oases", "oasis"),
        ("neuroses", "neurosis"),
        ("emphases", "emphasis"),
        ("tornadoes", "tornado"),
        ("mottoes", "motto"),
        ("dominoes", "domino"),
        ("hippopotami", "hippopotamus"),
        ("mongooses", "mongoose"),
        ("opuses", "opus"),
        ("occiputs", "occiput"),
        ("ganglions", "ganglion"),
        ("avalanches", "avalanche"),
        ("blouses", "blouse"),
        ("abuses", "abuse"),
        ("multimedia", "multimedia"),
        ("equipment", "equipment"),
        ("fish", "fish"),
        ("sheep", "sheep"),
        ("species", "species"),
        ("information", "information"),
        ("money", "money"),
        ("rice", "rice"),
        ("clothes", "clothes"),
        ("pants", "pants"),
        ("trivia", "trivia"),
        ("aircraft", "aircraft"),
        ("police", "police"),
        ("progress", "progress"),
        ("business", "business"),
        ("photo", "photo"),
        ("address", "address"),
        ("status", "status"),
        ("campus", "campus"),
        ("alias", "alias"),
        ("s", ""),
        ("es", "e"),
        ("", ""),
    ];

    for (plural, singular) in CASES {
        assert_eq!(
            &singularize_english_word(plural),
            singular,
            "singular of `{plural}`"
        );
    }
}

#[test]
fn singularizing_short_and_empty_words_does_not_panic() {
    assert_eq!(singularize_english_word(""), "");
    assert_eq!(singularize_english_word("s"), "");
    assert_eq!(singularize_english_word("es"), "e");
}

/// A hyphenated resource name singularizes only its last word; turning the
/// hyphens into underscores is the caller's job.
#[test]
fn singularizes_only_the_final_word_of_a_compound() {
    assert_eq!(singularize_english_word("blog-posts"), "blog-post");
}

/// `Pluralizer` carries the input's capitalization over to the result, so a
/// capitalized resource name keeps its shape in the derived wildcard.
#[test]
fn singularizing_preserves_the_case_of_the_input() {
    assert_eq!(singularize_english_word("Photos"), "Photo");
    assert_eq!(singularize_english_word("PHOTOS"), "PHOTO");
    assert_eq!(singularize_english_word("Categories"), "Category");
    assert_eq!(singularize_english_word("People"), "Person");
    assert_eq!(singularize_english_word("CHILDREN"), "CHILD");
    assert_eq!(singularize_english_word("Blog-Posts"), "Blog-Post");
}

/// Both tables are searched with `binary_search`, which silently returns the
/// wrong answer if they are not sorted.
#[test]
fn lookup_tables_are_sorted() {
    assert!(UNINFLECTED_WORDS.windows(2).all(|w| w[0] < w[1]));
    assert!(IRREGULAR_SINGULARS.windows(2).all(|w| w[0].0 < w[1].0));
}

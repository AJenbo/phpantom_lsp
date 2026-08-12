//! Type transformations: name resolution, self-substitution, generics.

use super::*;

/// What `static` / `$this` in a return type bind to once the call they were
/// read for is known.
#[derive(Debug, Clone, Copy)]
enum LsbBinding<'a> {
    /// Bind them over whatever class the replacement type names.
    Inherit,
    /// Bind them over `class`, the class the forwarding call is made from.
    Over(&'a str),
    /// Collapse them: the called class is statically fixed, so late static
    /// binding has nothing left to resolve.
    Fixed,
}

impl LsbBinding<'_> {
    /// The class to bind a `static` / `$this` keyword over, or `None` when the
    /// keyword should collapse to the replacement instead.
    ///
    /// A replacement that is not a plain name (a generic receiver such as
    /// `Builder<Article>`) carries no bound of its own, so `Inherit` collapses
    /// there too and the whole replacement stands in.
    fn bound_over(self, replacement: &PhpType) -> Option<Atom> {
        match self {
            LsbBinding::Inherit => match replacement.kind() {
                TypeKind::Named(name) => Some(*name),
                _ => None,
            },
            LsbBinding::Over(class) => Some(atom(class)),
            LsbBinding::Fixed => None,
        }
    }
}

impl PhpType {
    /// Produce a new `PhpType` with all class names resolved through
    /// the provided callback.
    ///
    /// The callback receives each class-like name (from `Named`,
    /// `Generic`, `ClassString`, etc.) and returns the resolved
    /// fully-qualified name. Names that are keywords/scalars are
    /// never passed to the callback.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let ty = PhpType::parse("Collection<int, User>|null");
    /// let resolved = ty.resolve_names(&|name| {
    ///     use_map.get(name).cloned()
    ///         .unwrap_or_else(|| format!("App\\{}", name))
    /// });
    /// // → Generic("App\\Collection", [Named("int"), Named("App\\User")]) | Named("null")
    /// ```
    pub fn resolve_names(&self, resolver: &dyn Fn(&str) -> String) -> PhpType {
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => PhpType::benevolent(inner.resolve_names(resolver)),
            TypeKind::Named(s) => {
                if is_keyword_type(s) {
                    PhpType::named(*s)
                } else {
                    PhpType::named(atom(&resolver(s)))
                }
            }

            TypeKind::Nullable(inner) => PhpType::nullable(inner.resolve_names(resolver)),

            TypeKind::Union(types) => {
                PhpType::union(types.iter().map(|t| t.resolve_names(resolver)).collect())
            }

            TypeKind::Intersection(types) => {
                PhpType::intersection(types.iter().map(|t| t.resolve_names(resolver)).collect())
            }

            TypeKind::Generic(g) => {
                let resolved_name = if is_keyword_type(&g.name) {
                    g.name
                } else {
                    atom(&resolver(&g.name))
                };
                PhpType::generic_atom(
                    resolved_name,
                    g.args.iter().map(|a| a.resolve_names(resolver)).collect(),
                )
            }

            TypeKind::Array(inner) => PhpType::array_of(inner.resolve_names(resolver)),

            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.resolve_names(resolver),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.resolve_names(resolver),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: if is_keyword_type(&c.kind) {
                    c.kind
                } else {
                    atom(&resolver(&c.kind))
                },
                params: c
                    .params
                    .iter()
                    .map(|p| CallableParam {
                        type_hint: p.type_hint.resolve_names(resolver),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c.return_type.as_ref().map(|rt| rt.resolve_names(resolver)),
            }),

            TypeKind::Conditional(c) => PhpType::conditional_type(ConditionalType {
                param: c.param,
                negated: c.negated,
                condition: c.condition.resolve_names(resolver),
                then_type: c.then_type.resolve_names(resolver),
                else_type: c.else_type.resolve_names(resolver),
            }),

            TypeKind::ClassString(inner) => {
                PhpType::class_string(inner.as_ref().map(|i| i.resolve_names(resolver)))
            }

            TypeKind::InterfaceString(inner) => {
                PhpType::interface_string(inner.as_ref().map(|i| i.resolve_names(resolver)))
            }

            TypeKind::KeyOf(inner) => PhpType::key_of(inner.resolve_names(resolver)),

            TypeKind::ValueOf(inner) => PhpType::value_of(inner.resolve_names(resolver)),

            TypeKind::IntRange(..) => self.clone(),

            TypeKind::IndexAccess(target, index) => PhpType::index_access(
                target.resolve_names(resolver),
                index.resolve_names(resolver),
            ),

            // Literals and raw types can't be structurally resolved.
            TypeKind::Literal(_) | TypeKind::Raw(_) => self.clone(),

            TypeKind::StaticType(s) => PhpType::static_type(atom(&resolver(s))),
            TypeKind::ThisType(s) => PhpType::this_type(atom(&resolver(s))),
        }
    }

    /// Return the short (unqualified) name from a potentially
    /// namespace-qualified type name. Returns only the part after the
    /// last `\`. Non-class types pass through unchanged.
    pub(crate) fn short_name_of(name: &str) -> &str {
        crate::util::short_name(name.trim())
    }

    /// Produce a new `PhpType` with all namespace-qualified names
    /// shortened to their unqualified form.
    ///
    /// For example, `App\Models\User|null` becomes `User|null`, and
    /// `array<int, App\Models\User>` becomes `array<int, User>`.
    pub fn shorten(&self) -> PhpType {
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => PhpType::benevolent(inner.shorten()),
            TypeKind::Named(s) => PhpType::named(atom(Self::short_name_of(s))),

            TypeKind::Nullable(inner) => PhpType::nullable(inner.shorten()),

            TypeKind::Union(types) => PhpType::union(types.iter().map(|t| t.shorten()).collect()),

            TypeKind::Intersection(types) => {
                PhpType::intersection(types.iter().map(|t| t.shorten()).collect())
            }

            TypeKind::Generic(g) => PhpType::generic(
                Self::short_name_of(&g.name),
                g.args.iter().map(|a| a.shorten()).collect(),
            ),

            TypeKind::Array(inner) => PhpType::array_of(inner.shorten()),

            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.shorten(),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.shorten(),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: atom(Self::short_name_of(&c.kind)),
                params: c
                    .params
                    .iter()
                    .map(|p| CallableParam {
                        type_hint: p.type_hint.shorten(),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c.return_type.as_ref().map(|rt| rt.shorten()),
            }),

            TypeKind::Conditional(c) => PhpType::conditional_type(ConditionalType {
                param: c.param,
                negated: c.negated,
                condition: c.condition.shorten(),
                then_type: c.then_type.shorten(),
                else_type: c.else_type.shorten(),
            }),

            TypeKind::ClassString(inner) => {
                PhpType::class_string(inner.as_ref().map(|i| i.shorten()))
            }

            TypeKind::InterfaceString(inner) => {
                PhpType::interface_string(inner.as_ref().map(|i| i.shorten()))
            }

            TypeKind::KeyOf(inner) => PhpType::key_of(inner.shorten()),

            TypeKind::ValueOf(inner) => PhpType::value_of(inner.shorten()),

            TypeKind::IntRange(..) => self.clone(),

            TypeKind::IndexAccess(target, index) => {
                PhpType::index_access(target.shorten(), index.shorten())
            }

            // Literals carry no class name; raw types cannot be
            // structurally shortened.
            TypeKind::Literal(_) | TypeKind::Raw(_) => self.clone(),

            TypeKind::StaticType(s) => PhpType::static_type(atom(Self::short_name_of(s))),
            TypeKind::ThisType(s) => PhpType::this_type(atom(Self::short_name_of(s))),
        }
    }

    /// Produce a new `PhpType` with `self`, `static`, and `$this`
    /// replaced by the given class name.
    ///
    /// Walks the entire type tree and replaces any `Named("self")`,
    /// `Named("static")`, or `Named("$this")` with
    /// `Named(class_name)`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let ty = PhpType::parse("self|null");
    /// let replaced = ty.replace_self("App\\User");
    /// assert_eq!(replaced.to_string(), "App\\User | null");
    /// ```
    pub fn replace_self(&self, class_name: &str) -> PhpType {
        self.replace_self_with_type(&PhpType::named(atom(class_name)))
    }

    /// Resolve relative class-reference keywords to concrete class names,
    /// walking the entire type tree (including array elements and generic
    /// arguments).
    ///
    /// `self`, `static`, and `$this` become `class_name`; `parent` becomes
    /// `parent_class` when it is `Some`.  Unlike [`resolve_names`], which
    /// treats these keywords as non-class types and leaves them untouched,
    /// this resolves them so a declared type can be compared against a
    /// resolved value type.
    ///
    /// [`resolve_names`]: PhpType::resolve_names
    pub fn resolve_self_refs(&self, class_name: &str, parent_class: Option<&str>) -> PhpType {
        self.resolve_self_refs_bounded(class_name, parent_class)
    }

    /// Like [`resolve_self_refs`] but produces bounded static types:
    /// `static` → [`StaticType(bound)`](TypeKind::StaticType),
    /// `$this` → [`ThisType(bound)`](TypeKind::ThisType),
    /// `self` → [`Named(class_name)`](TypeKind::Named),
    /// `parent` → [`Named(parent_class)`](TypeKind::Named).
    ///
    /// Use this when the caller needs to preserve the late-static-binding
    /// distinction rather than flattening everything to a concrete class.
    pub fn resolve_self_refs_bounded(
        &self,
        class_name: &str,
        parent_class: Option<&str>,
    ) -> PhpType {
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => {
                PhpType::benevolent(inner.resolve_self_refs_bounded(class_name, parent_class))
            }
            TypeKind::Named(s) if is_self_ref_name(s) || s.eq_ignore_ascii_case("parent") => {
                if s.eq_ignore_ascii_case("static") {
                    PhpType::static_type(atom(class_name))
                } else if s.eq_ignore_ascii_case("$this") {
                    PhpType::this_type(atom(class_name))
                } else if s.eq_ignore_ascii_case("parent") {
                    match parent_class {
                        Some(p) => PhpType::named(atom(p)),
                        None => self.clone(),
                    }
                } else {
                    PhpType::named(atom(class_name))
                }
            }
            TypeKind::Named(_)
            | TypeKind::StaticType(_)
            | TypeKind::ThisType(_)
            | TypeKind::Literal(_)
            | TypeKind::Raw(_)
            | TypeKind::IntRange(..) => self.clone(),
            TypeKind::Nullable(inner) => {
                PhpType::nullable(inner.resolve_self_refs_bounded(class_name, parent_class))
            }
            TypeKind::Union(types) => PhpType::union(
                types
                    .iter()
                    .map(|t| t.resolve_self_refs_bounded(class_name, parent_class))
                    .collect(),
            ),
            TypeKind::Intersection(types) => PhpType::intersection(
                types
                    .iter()
                    .map(|t| t.resolve_self_refs_bounded(class_name, parent_class))
                    .collect(),
            ),
            TypeKind::Generic(g) => {
                let resolved_name = if is_self_ref_name(&g.name) {
                    atom(class_name)
                } else if g.name.eq_ignore_ascii_case("parent") {
                    parent_class.map(atom).unwrap_or(g.name)
                } else {
                    g.name
                };
                PhpType::generic_atom(
                    resolved_name,
                    g.args
                        .iter()
                        .map(|a| a.resolve_self_refs_bounded(class_name, parent_class))
                        .collect(),
                )
            }
            TypeKind::Array(inner) => {
                PhpType::array_of(inner.resolve_self_refs_bounded(class_name, parent_class))
            }
            TypeKind::ClassString(inner) => PhpType::class_string(
                inner
                    .as_ref()
                    .map(|t| t.resolve_self_refs_bounded(class_name, parent_class)),
            ),
            TypeKind::InterfaceString(inner) => PhpType::interface_string(
                inner
                    .as_ref()
                    .map(|t| t.resolve_self_refs_bounded(class_name, parent_class)),
            ),
            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| super::ShapeEntry {
                        key: e.key.clone(),
                        value_type: e
                            .value_type
                            .resolve_self_refs_bounded(class_name, parent_class),
                        optional: e.optional,
                    })
                    .collect(),
            ),
            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| super::ShapeEntry {
                        key: e.key.clone(),
                        value_type: e
                            .value_type
                            .resolve_self_refs_bounded(class_name, parent_class),
                        optional: e.optional,
                    })
                    .collect(),
            ),
            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: c.kind,
                params: c
                    .params
                    .iter()
                    .map(|p| super::CallableParam {
                        type_hint: p
                            .type_hint
                            .resolve_self_refs_bounded(class_name, parent_class),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c
                    .return_type
                    .as_ref()
                    .map(|r| r.resolve_self_refs_bounded(class_name, parent_class)),
            }),
            TypeKind::Conditional(c) => PhpType::conditional_type(ConditionalType {
                param: c.param,
                negated: c.negated,
                condition: c
                    .condition
                    .resolve_self_refs_bounded(class_name, parent_class),
                then_type: c
                    .then_type
                    .resolve_self_refs_bounded(class_name, parent_class),
                else_type: c
                    .else_type
                    .resolve_self_refs_bounded(class_name, parent_class),
            }),
            TypeKind::KeyOf(inner) => {
                PhpType::key_of(inner.resolve_self_refs_bounded(class_name, parent_class))
            }
            TypeKind::ValueOf(inner) => {
                PhpType::value_of(inner.resolve_self_refs_bounded(class_name, parent_class))
            }
            TypeKind::IndexAccess(base, index) => PhpType::index_access(
                base.resolve_self_refs_bounded(class_name, parent_class),
                index.resolve_self_refs_bounded(class_name, parent_class),
            ),
        }
    }

    /// Replace only the `self` keyword (not `static` or `$this`) with a
    /// concrete class name.  Used during inheritance merging so that
    /// inherited methods carry the declaring class's identity for `self`
    /// while preserving `static` for late-static-binding resolution.
    pub fn replace_bare_self(&self, class_name: &str) -> PhpType {
        self.replace_bare_keyword("self", class_name)
    }

    /// Replace only the `parent` keyword with a concrete class name.
    ///
    /// `parent` binds to the parent of the class that *declares* the method,
    /// so an inherited method must carry that class rather than the keyword:
    /// resolving `parent` against the class the call is made on names the
    /// wrong class as soon as another subclass is in between.
    pub fn replace_bare_parent(&self, class_name: &str) -> PhpType {
        self.replace_bare_keyword("parent", class_name)
    }

    fn replace_bare_keyword(&self, keyword: &str, class_name: &str) -> PhpType {
        if let TypeKind::Benevolent(inner) = self.raw_kind() {
            return PhpType::benevolent(inner.replace_bare_keyword(keyword, class_name));
        }
        match self.kind() {
            TypeKind::Named(s) if s.eq_ignore_ascii_case(keyword) => {
                PhpType::named(atom(class_name))
            }
            TypeKind::Named(_) | TypeKind::Literal(_) | TypeKind::Raw(_) => self.clone(),
            TypeKind::Nullable(inner) => {
                PhpType::nullable(inner.replace_bare_keyword(keyword, class_name))
            }
            TypeKind::Union(types) => PhpType::union(
                types
                    .iter()
                    .map(|t| t.replace_bare_keyword(keyword, class_name))
                    .collect(),
            ),
            TypeKind::Intersection(types) => PhpType::intersection(
                types
                    .iter()
                    .map(|t| t.replace_bare_keyword(keyword, class_name))
                    .collect(),
            ),
            TypeKind::Generic(g) => {
                let resolved_name = if g.name.eq_ignore_ascii_case(keyword) {
                    atom(class_name)
                } else {
                    g.name
                };
                PhpType::generic_atom(
                    resolved_name,
                    g.args
                        .iter()
                        .map(|a| a.replace_bare_keyword(keyword, class_name))
                        .collect(),
                )
            }
            TypeKind::Array(inner) => {
                PhpType::array_of(inner.replace_bare_keyword(keyword, class_name))
            }
            _ => self.clone(),
        }
    }

    /// Returns `true` when this type contains the bare `self` keyword
    /// (not `static` or `$this`).
    pub fn contains_bare_self(&self) -> bool {
        self.contains_bare_keyword("self")
    }

    /// Returns `true` when this type contains the bare `parent` keyword.
    pub fn contains_bare_parent(&self) -> bool {
        self.contains_bare_keyword("parent")
    }

    fn contains_bare_keyword(&self, keyword: &str) -> bool {
        match self.kind() {
            TypeKind::Named(s) => s.eq_ignore_ascii_case(keyword),
            TypeKind::Nullable(inner) => inner.contains_bare_keyword(keyword),
            TypeKind::Union(types) | TypeKind::Intersection(types) => {
                types.iter().any(|t| t.contains_bare_keyword(keyword))
            }
            TypeKind::Generic(g) => {
                g.name.eq_ignore_ascii_case(keyword)
                    || g.args.iter().any(|a| a.contains_bare_keyword(keyword))
            }
            TypeKind::Array(inner) => inner.contains_bare_keyword(keyword),
            _ => false,
        }
    }

    /// Check whether this type tree contains any `self`, `static`, or
    /// `$this` references that [`replace_self`] / [`replace_self_with_type`]
    /// would replace.
    pub fn contains_self_ref(&self) -> bool {
        self.contains_name_matching(&is_self_ref_name)
    }

    /// Check whether this type tree contains any relative class-reference
    /// keyword: `self`, `static`, `$this`, or `parent`.
    ///
    /// This is the gate for [`resolve_self_refs_bounded`], which resolves
    /// all four. [`contains_self_ref`] omits `parent`, so using it as the
    /// gate leaves a `parent` type hint unresolved.
    ///
    /// [`resolve_self_refs_bounded`]: PhpType::resolve_self_refs_bounded
    /// [`contains_self_ref`]: PhpType::contains_self_ref
    pub fn contains_relative_class_ref(&self) -> bool {
        self.contains_name_matching(&|name| {
            is_self_ref_name(name) || name.eq_ignore_ascii_case("parent")
        })
    }

    /// Replace every [`TypeKind::Conditional`] in this type tree with the
    /// union of its branches.
    ///
    /// A conditional whose condition has not been decided still describes a
    /// value that satisfies one of its two branches, so `then|else` is the
    /// tightest type that holds however the condition resolves — and, when
    /// both branches are the same type, simply that type.  The raw
    /// conditional is a type *expression*, not a set of values, so anything
    /// that compares a concrete type against it (argument compatibility,
    /// for instance) must collapse it first or nothing will ever match.
    ///
    /// Prefer evaluating the condition against the call's arguments when
    /// they are available; this is the fallback for when they are not.
    pub fn conditionals_as_branch_unions(&self) -> PhpType {
        if !self.contains_conditional() {
            return self.clone();
        }
        let recurse = |inner: &PhpType| inner.conditionals_as_branch_unions();
        if let TypeKind::Benevolent(inner) = self.raw_kind() {
            return PhpType::benevolent(recurse(inner));
        }
        match self.kind() {
            TypeKind::Conditional(c) => {
                let mut members: Vec<PhpType> = Vec::new();
                for branch in [&c.then_type, &c.else_type] {
                    for member in recurse(branch).union_members() {
                        if !members.contains(member) {
                            members.push(member.clone());
                        }
                    }
                }
                match members.len() {
                    1 => members.into_iter().next().expect("checked length"),
                    _ => PhpType::union(members),
                }
            }
            TypeKind::Nullable(inner) => PhpType::nullable(recurse(inner)),
            TypeKind::Array(inner) => PhpType::array_of(recurse(inner)),
            TypeKind::Union(members) => PhpType::union(members.iter().map(recurse).collect()),
            TypeKind::Intersection(members) => {
                PhpType::intersection(members.iter().map(recurse).collect())
            }
            TypeKind::Generic(g) => {
                PhpType::generic_atom(g.name, g.args.iter().map(recurse).collect())
            }
            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: recurse(&e.value_type),
                        optional: e.optional,
                    })
                    .collect(),
            ),
            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: recurse(&e.value_type),
                        optional: e.optional,
                    })
                    .collect(),
            ),
            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: c.kind,
                params: c
                    .params
                    .iter()
                    .map(|p| CallableParam {
                        type_hint: recurse(&p.type_hint),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c.return_type.as_ref().map(recurse),
            }),
            TypeKind::ClassString(inner) => PhpType::class_string(inner.as_ref().map(recurse)),
            TypeKind::InterfaceString(inner) => {
                PhpType::interface_string(inner.as_ref().map(recurse))
            }
            TypeKind::KeyOf(inner) => PhpType::key_of(recurse(inner)),
            TypeKind::ValueOf(inner) => PhpType::value_of(recurse(inner)),
            TypeKind::IndexAccess(base, index) => {
                PhpType::index_access(recurse(base), recurse(index))
            }
            _ => self.clone(),
        }
    }

    /// Replace every type operator that never got evaluated with the widest
    /// type its result can have: `key-of<T>` with `array-key`, `value-of<T>`
    /// and `T[K]` with `mixed`.
    ///
    /// `key-of<array{a: int}>` is evaluated the moment it is parsed; what
    /// survives is the form whose operand we could not read (a class constant,
    /// a template that was never substituted).  That leftover is a type
    /// *expression*, not a set of values, so anything comparing a concrete type
    /// against it — argument compatibility above all — has to widen it first or
    /// every value, valid or not, comes back as a mismatch.  The bounds are the
    /// ones PHPStan falls back to: an array key is an `int|string` whichever
    /// key it turns out to be, and a value can be anything.
    pub fn unevaluated_operators_as_bounds(&self) -> PhpType {
        if !self.contains_unevaluated_operator() {
            return self.clone();
        }
        let recurse = |inner: &PhpType| inner.unevaluated_operators_as_bounds();
        if let TypeKind::Benevolent(inner) = self.raw_kind() {
            return PhpType::benevolent(recurse(inner));
        }
        match self.kind() {
            TypeKind::KeyOf(_) => PhpType::named(atom("array-key")),
            TypeKind::ValueOf(_) | TypeKind::IndexAccess(..) => PhpType::mixed(),
            TypeKind::Nullable(inner) => PhpType::nullable(recurse(inner)),
            TypeKind::Array(inner) => PhpType::array_of(recurse(inner)),
            TypeKind::Union(members) => PhpType::union(members.iter().map(recurse).collect()),
            TypeKind::Intersection(members) => {
                PhpType::intersection(members.iter().map(recurse).collect())
            }
            TypeKind::Generic(g) => {
                PhpType::generic_atom(g.name, g.args.iter().map(recurse).collect())
            }
            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: recurse(&e.value_type),
                        optional: e.optional,
                    })
                    .collect(),
            ),
            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: recurse(&e.value_type),
                        optional: e.optional,
                    })
                    .collect(),
            ),
            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: c.kind,
                params: c
                    .params
                    .iter()
                    .map(|p| CallableParam {
                        type_hint: recurse(&p.type_hint),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c.return_type.as_ref().map(recurse),
            }),
            TypeKind::ClassString(inner) => PhpType::class_string(inner.as_ref().map(recurse)),
            TypeKind::InterfaceString(inner) => {
                PhpType::interface_string(inner.as_ref().map(recurse))
            }
            TypeKind::Conditional(c) => PhpType::conditional_type(ConditionalType {
                param: c.param,
                negated: c.negated,
                condition: recurse(&c.condition),
                then_type: recurse(&c.then_type),
                else_type: recurse(&c.else_type),
            }),
            _ => self.clone(),
        }
    }

    /// Walk the type tree looking for a named type whose name satisfies
    /// `pred`.
    fn contains_name_matching(&self, pred: &dyn Fn(&str) -> bool) -> bool {
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => inner.contains_name_matching(pred),
            TypeKind::Named(s) => pred(s),
            TypeKind::Nullable(inner) => inner.contains_name_matching(pred),
            TypeKind::Union(types) | TypeKind::Intersection(types) => {
                types.iter().any(|t| t.contains_name_matching(pred))
            }
            TypeKind::Generic(g) => {
                pred(&g.name) || g.args.iter().any(|a| a.contains_name_matching(pred))
            }
            TypeKind::Array(inner) => inner.contains_name_matching(pred),
            TypeKind::ArrayShape(entries) | TypeKind::ObjectShape(entries) => entries
                .iter()
                .any(|e| e.value_type.contains_name_matching(pred)),
            TypeKind::Callable(c) => {
                c.params
                    .iter()
                    .any(|p| p.type_hint.contains_name_matching(pred))
                    || c.return_type
                        .as_ref()
                        .is_some_and(|r| r.contains_name_matching(pred))
            }
            TypeKind::Conditional(c) => {
                c.condition.contains_name_matching(pred)
                    || c.then_type.contains_name_matching(pred)
                    || c.else_type.contains_name_matching(pred)
            }
            TypeKind::ClassString(inner) | TypeKind::InterfaceString(inner) => inner
                .as_ref()
                .is_some_and(|t| t.contains_name_matching(pred)),
            TypeKind::KeyOf(inner) | TypeKind::ValueOf(inner) => inner.contains_name_matching(pred),
            TypeKind::IndexAccess(base, index) => {
                base.contains_name_matching(pred) || index.contains_name_matching(pred)
            }
            TypeKind::StaticType(_) | TypeKind::ThisType(_) => false,
            TypeKind::Literal(_) | TypeKind::Raw(_) | TypeKind::IntRange(_, _) => false,
        }
    }

    /// Replace `self` / `static` / `$this` throughout this type tree
    /// with the given [`PhpType`].
    ///
    /// This is the structured counterpart of [`replace_self`]: instead of
    /// replacing with a bare class name (`PhpType::named(name)`), it
    /// substitutes a full type expression.  This preserves generic
    /// parameters when the receiver is a generic type like
    /// `Builder<Article>`.
    ///
    /// When `replacement` is `TypeKind::Generic("Builder", [Named("Article")])`
    /// and the return type is `Named("static")`, the result is the full
    /// generic type.  When the return type is `Generic("static", [args])`,
    /// the replacement's base name is used and the return type's own args
    /// are kept (they override the receiver's args).
    pub fn replace_self_with_type(&self, replacement: &PhpType) -> PhpType {
        self.replace_self_inner(replacement, LsbBinding::Inherit)
    }

    /// Replace `self` / `static` / `$this` throughout this type tree, with
    /// explicit control over what the late-static-binding keywords bind to.
    ///
    /// `self` always becomes `self_class`, the class whose declaration the
    /// annotation was read from, because `self` is invariant.  `static` and
    /// `$this` become a bounded type over `lsb_class`, the class the call is
    /// made *from*, which is not always the same class: `parent::create()` in
    /// `B extends A` reads the annotation off `A` but still resolves `static`
    /// to `B`.
    ///
    /// Pass `None` for `lsb_class` when the called class is statically fixed —
    /// an explicit `A::create()` on a `static` method, or `new A`.  PHP
    /// resolves `static` to exactly `A` there however `A` is subclassed, so a
    /// bounded [`StaticType`](TypeKind::StaticType) would claim an openness
    /// the call does not have.
    pub fn replace_self_bound(&self, self_class: &str, lsb_class: Option<&str>) -> PhpType {
        let lsb = match lsb_class {
            Some(class) => LsbBinding::Over(class),
            None => LsbBinding::Fixed,
        };
        self.replace_self_inner(&PhpType::named(atom(self_class)), lsb)
    }

    fn replace_self_inner(&self, replacement: &PhpType, lsb: LsbBinding<'_>) -> PhpType {
        // Extract the base class name from the replacement for use in
        // Generic nodes where only the name part is replaced.
        let replacement_name = match replacement.kind() {
            TypeKind::Named(n) | TypeKind::StaticType(n) | TypeKind::ThisType(n) => n.as_str(),
            TypeKind::Generic(g) => g.name.as_str(),
            _ => "",
        };
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => {
                PhpType::benevolent(inner.replace_self_inner(replacement, lsb))
            }
            TypeKind::Named(s) if self.is_self_ref() => {
                let Some(bound) = lsb.bound_over(replacement) else {
                    return replacement.clone();
                };
                if s.eq_ignore_ascii_case("static") {
                    PhpType::static_type(bound)
                } else if s.eq_ignore_ascii_case("$this") {
                    PhpType::this_type(bound)
                } else {
                    replacement.clone()
                }
            }

            TypeKind::Named(_) | TypeKind::Literal(_) | TypeKind::Raw(_) => self.clone(),

            TypeKind::Nullable(inner) => {
                PhpType::nullable(inner.replace_self_inner(replacement, lsb))
            }

            TypeKind::Union(types) => PhpType::union(
                types
                    .iter()
                    .map(|t| t.replace_self_inner(replacement, lsb))
                    .collect(),
            ),

            TypeKind::Intersection(types) => PhpType::intersection(
                types
                    .iter()
                    .map(|t| t.replace_self_inner(replacement, lsb))
                    .collect(),
            ),

            TypeKind::Generic(g) => {
                let resolved_name = if is_self_ref_name(&g.name) {
                    atom(replacement_name)
                } else {
                    g.name
                };
                PhpType::generic_atom(
                    resolved_name,
                    g.args
                        .iter()
                        .map(|a| a.replace_self_inner(replacement, lsb))
                        .collect(),
                )
            }

            TypeKind::Array(inner) => PhpType::array_of(inner.replace_self_inner(replacement, lsb)),

            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.replace_self_inner(replacement, lsb),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.replace_self_inner(replacement, lsb),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: c.kind,
                params: c
                    .params
                    .iter()
                    .map(|p| CallableParam {
                        type_hint: p.type_hint.replace_self_inner(replacement, lsb),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c
                    .return_type
                    .as_ref()
                    .map(|r| r.replace_self_inner(replacement, lsb)),
            }),

            TypeKind::Conditional(c) => PhpType::conditional_type(ConditionalType {
                param: c.param,
                negated: c.negated,
                condition: c.condition.replace_self_inner(replacement, lsb),
                then_type: c.then_type.replace_self_inner(replacement, lsb),
                else_type: c.else_type.replace_self_inner(replacement, lsb),
            }),

            TypeKind::ClassString(inner) => PhpType::class_string(
                inner
                    .as_ref()
                    .map(|t| t.replace_self_inner(replacement, lsb)),
            ),

            TypeKind::InterfaceString(inner) => PhpType::interface_string(
                inner
                    .as_ref()
                    .map(|t| t.replace_self_inner(replacement, lsb)),
            ),

            TypeKind::KeyOf(inner) => PhpType::key_of(inner.replace_self_inner(replacement, lsb)),

            TypeKind::ValueOf(inner) => {
                PhpType::value_of(inner.replace_self_inner(replacement, lsb))
            }

            TypeKind::IntRange(..) => self.clone(),

            TypeKind::IndexAccess(base, index) => PhpType::index_access(
                base.replace_self_inner(replacement, lsb),
                index.replace_self_inner(replacement, lsb),
            ),

            // A bound already applied by an earlier hop still answers to a
            // fixed call target: `A::create()` pins whatever `create()` left
            // open.
            TypeKind::StaticType(n) | TypeKind::ThisType(n) if matches!(lsb, LsbBinding::Fixed) => {
                PhpType::named(*n)
            }

            TypeKind::StaticType(_) | TypeKind::ThisType(_) => self.clone(),
        }
    }

    /// Substitute template parameter names throughout this type tree.
    ///
    /// Walks the entire type tree and replaces any `Named(s)` node whose
    /// name appears as a key in `subs` with the corresponding `PhpType`.
    /// All other nodes are recursively rebuilt with their children
    /// substituted.
    ///
    /// A `Raw` node — text no type syntax covers, such as the `Foo::BAR`
    /// spelling of a class constant — is looked up the same way. It is only
    /// opaque because nothing has told us what it means; when `subs` does,
    /// that reading wins.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::collections::HashMap;
    /// let ty = PhpType::parse("Collection<TKey, TValue>");
    /// let subs: HashMap<String, PhpType> =
    ///     [("TKey".into(), PhpType::parse("int")), ("TValue".into(), PhpType::parse("User"))]
    ///         .into_iter().collect();
    /// let result = ty.substitute(&subs);
    /// assert_eq!(result.to_string(), "Collection<int, User>");
    /// ```
    pub fn substitute(&self, subs: &std::collections::HashMap<String, PhpType>) -> PhpType {
        if subs.is_empty() {
            return self.clone();
        }
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => PhpType::benevolent(inner.substitute(subs)),
            TypeKind::Named(s) => {
                if let Some(replacement) = subs.get(s.as_str()) {
                    replacement.clone()
                } else {
                    self.clone()
                }
            }

            TypeKind::Raw(s) => match subs.get(s.as_ref()) {
                Some(replacement) => replacement.clone(),
                None => self.clone(),
            },

            TypeKind::Literal(_) | TypeKind::IntRange(_, _) => self.clone(),

            TypeKind::StaticType(_) | TypeKind::ThisType(_) => self.clone(),

            TypeKind::Nullable(inner) => {
                let resolved = inner.substitute(subs);
                // If the substitution produced a union or nullable,
                // don't double-wrap.
                match &resolved.kind() {
                    TypeKind::Nullable(_) => resolved,
                    TypeKind::Union(members) => {
                        // Already nullable if it contains null
                        if members.iter().any(
                            |m| matches!(m.kind(), TypeKind::Named(n) if n.eq_ignore_ascii_case("null")),
                        ) {
                            resolved
                        } else {
                            PhpType::nullable(resolved)               }
                    }
                    _ => PhpType::nullable(resolved),
                }
            }

            TypeKind::Union(types) => {
                let resolved: Vec<PhpType> = types.iter().map(|t| t.substitute(subs)).collect();
                // Flatten any nested unions produced by substitution.
                let mut flat = Vec::with_capacity(resolved.len());
                for t in resolved {
                    match t.kind() {
                        TypeKind::Union(inner) => flat.extend(inner.iter().cloned()),
                        _ => flat.push(t),
                    }
                }
                if flat.len() == 1 {
                    flat.into_iter().next().unwrap()
                } else {
                    PhpType::union(flat)
                }
            }

            TypeKind::Intersection(types) => {
                let resolved: Vec<PhpType> = types.iter().map(|t| t.substitute(subs)).collect();
                let mut flat = Vec::with_capacity(resolved.len());
                for t in resolved {
                    match t.kind() {
                        TypeKind::Intersection(inner) => flat.extend(inner.iter().cloned()),
                        _ => flat.push(t),
                    }
                }
                if flat.len() == 1 {
                    flat.into_iter().next().unwrap()
                } else {
                    PhpType::intersection(flat)
                }
            }

            TypeKind::Generic(g) => {
                // The base name might itself be a template parameter.
                if let Some(replacement) = subs.get(g.name.as_str()) {
                    match replacement.kind() {
                        TypeKind::Named(n) => PhpType::generic_atom(
                            *n,
                            g.args.iter().map(|a| a.substitute(subs)).collect(),
                        ),
                        // Use the replacement's base name but keep the
                        // original generic args (substituted).  The
                        // replacement's own args are discarded because
                        // the source type provides its own parameters.
                        TypeKind::Generic(base) => PhpType::generic_atom(
                            base.name,
                            g.args.iter().map(|a| a.substitute(subs)).collect(),
                        ),
                        // For non-class replacements (union, intersection,
                        // etc.), the generic wrapper is meaningless — return
                        // the replacement as-is.
                        _ => replacement.clone(),
                    }
                } else {
                    PhpType::generic_atom(
                        g.name,
                        g.args.iter().map(|a| a.substitute(subs)).collect(),
                    )
                }
            }

            TypeKind::Array(inner) => PhpType::array_of(inner.substitute(subs)),

            TypeKind::ArrayShape(entries) => PhpType::array_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.substitute(subs),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::ObjectShape(entries) => PhpType::object_shape(
                entries
                    .iter()
                    .map(|e| ShapeEntry {
                        key: e.key.clone(),
                        value_type: e.value_type.substitute(subs),
                        optional: e.optional,
                    })
                    .collect(),
            ),

            TypeKind::Callable(c) => PhpType::callable_type(CallableType {
                kind: c.kind,
                params: c
                    .params
                    .iter()
                    .map(|p| CallableParam {
                        type_hint: p.type_hint.substitute(subs),
                        optional: p.optional,
                        variadic: p.variadic,
                    })
                    .collect(),
                return_type: c.return_type.as_ref().map(|r| r.substitute(subs)),
            }),

            TypeKind::Conditional(c) => PhpType::conditional_type(ConditionalType {
                param: c.param,
                negated: c.negated,
                condition: c.condition.substitute(subs),
                then_type: c.then_type.substitute(subs),
                else_type: c.else_type.substitute(subs),
            }),

            TypeKind::ClassString(inner) => {
                PhpType::class_string(inner.as_ref().map(|t| t.substitute(subs)))
            }

            TypeKind::InterfaceString(inner) => {
                PhpType::interface_string(inner.as_ref().map(|t| t.substitute(subs)))
            }

            TypeKind::KeyOf(inner) => {
                let resolved = inner.substitute(subs);
                evaluate_key_of(&resolved)
            }

            TypeKind::ValueOf(inner) => {
                let resolved = inner.substitute(subs);
                evaluate_value_of(&resolved)
            }

            TypeKind::IndexAccess(base, index) => {
                let resolved_base = base.substitute(subs);
                let resolved_index = index.substitute(subs);
                evaluate_index_access(&resolved_base, &resolved_index)
            }
        }
    }

    /// Extract all class-like names from this type, recursively.
    ///
    /// Walks the entire type tree and collects the base names of all
    /// class-like types (including those nested inside generics,
    /// callables, shapes, etc.). Scalar types, keywords, `null`,
    /// and literals are skipped.
    ///
    /// For `Collection<int, User>|null`, returns `["Collection", "User"]`.
    /// For `?User`, returns `["User"]`.
    /// For `int|string`, returns `[]`.
    pub fn extract_class_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.collect_class_names(&mut names);
        names
    }

    /// Extract only top-level class names from this type.
    ///
    /// Unlike [`extract_class_names`], this does **not** recurse into
    /// generic type arguments, callable parameters, shape entries, or
    /// other nested positions. It returns only the outermost class
    /// names that are directly part of the type expression.
    ///
    /// For `Collection<int, User>|null`, returns `["Collection"]`.
    /// For `User|Admin`, returns `["User", "Admin"]`.
    /// For `?User`, returns `["User"]`.
    /// For `User[]`, returns `["User"]`.
    /// For `int|string`, returns `[]`.
    ///
    /// Go-to-type-definition uses this rather than [`extract_class_names`]
    /// because it should jump to the container class, not its type
    /// arguments.
    pub fn top_level_class_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.collect_top_level_class_names(&mut names);
        names
    }

    /// Recursive helper for [`extract_class_names`].
    fn collect_class_names(&self, names: &mut Vec<String>) {
        match self.raw_kind() {
            TypeKind::Benevolent(inner) => inner.collect_class_names(names),
            TypeKind::Named(s) => {
                if !is_keyword_type(s) && !s.is_empty() && !names.iter().any(|n| n == s.as_str()) {
                    names.push(s.to_string());
                }
            }

            TypeKind::Nullable(inner) => inner.collect_class_names(names),

            TypeKind::Union(types) | TypeKind::Intersection(types) => {
                for t in types {
                    t.collect_class_names(names);
                }
            }

            TypeKind::Generic(g) => {
                if !is_keyword_type(&g.name)
                    && !g.name.is_empty()
                    && !names.iter().any(|n| n == g.name.as_str())
                {
                    names.push(g.name.to_string());
                }
                for a in &g.args {
                    a.collect_class_names(names);
                }
            }

            TypeKind::Array(inner) => inner.collect_class_names(names),

            TypeKind::ArrayShape(entries) | TypeKind::ObjectShape(entries) => {
                for e in entries {
                    e.value_type.collect_class_names(names);
                }
            }

            TypeKind::Callable(c) => {
                for p in &c.params {
                    p.type_hint.collect_class_names(names);
                }
                if let Some(ret) = &c.return_type {
                    ret.collect_class_names(names);
                }
            }

            TypeKind::ClassString(inner) => {
                if let Some(t) = inner {
                    t.collect_class_names(names);
                }
            }

            TypeKind::InterfaceString(inner) => {
                if let Some(t) = inner {
                    t.collect_class_names(names);
                }
            }

            TypeKind::KeyOf(inner) | TypeKind::ValueOf(inner) => {
                inner.collect_class_names(names);
            }

            TypeKind::IndexAccess(base, index) => {
                base.collect_class_names(names);
                index.collect_class_names(names);
            }

            TypeKind::Conditional(c) => {
                c.condition.collect_class_names(names);
                c.then_type.collect_class_names(names);
                c.else_type.collect_class_names(names);
            }

            TypeKind::StaticType(s) | TypeKind::ThisType(s) => {
                if !s.is_empty() && !names.iter().any(|n| n == s.as_str()) {
                    names.push(s.to_string());
                }
            }

            TypeKind::Literal(_) | TypeKind::Raw(_) | TypeKind::IntRange(_, _) => {}
        }
    }

    /// Recursive helper for [`top_level_class_names`].
    ///
    /// Only descends through union, intersection, and nullable
    /// wrappers. Does not recurse into generic args, callable
    /// params/return, shapes, class-string inner types, etc.
    fn collect_top_level_class_names(&self, names: &mut Vec<String>) {
        match self.kind() {
            TypeKind::Named(s)
                if !is_keyword_type(s)
                    && !s.is_empty()
                    && !names.iter().any(|n| n == s.as_str()) =>
            {
                names.push(s.to_string());
            }

            TypeKind::Nullable(inner) => inner.collect_top_level_class_names(names),

            TypeKind::Union(types) | TypeKind::Intersection(types) => {
                for t in types {
                    t.collect_top_level_class_names(names);
                }
            }

            // For generics, only the base name is top-level.
            // `Collection<int, User>` → `["Collection"]`.
            TypeKind::Generic(g)
                if !is_keyword_type(&g.name)
                    && !g.name.is_empty()
                    && !names.iter().any(|n| n == g.name.as_str()) =>
            {
                names.push(g.name.to_string());
            }

            TypeKind::StaticType(s) | TypeKind::ThisType(s)
                if !s.is_empty() && !names.iter().any(|n| n == s.as_str()) =>
            {
                names.push(s.to_string());
            }

            // `User[]` — the inner type is the top-level class.
            TypeKind::Array(inner) => inner.collect_top_level_class_names(names),

            // Shapes, callables, class-string, key-of, value-of,
            // conditionals, literals, int-ranges — no navigable
            // top-level class name.
            _ => {}
        }
    }
}

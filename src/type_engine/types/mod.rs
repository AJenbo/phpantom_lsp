/// Type resolution sub-modules.
///
/// This group contains modules related to type resolution:
/// - **resolution**: Type-hint string to `ClassInfo` mapping (unions,
///   intersections, generics, type aliases, object shapes, property types)
/// - **narrowing**: instanceof / assert / custom type guard narrowing
/// - **conditional**: PHPStan conditional return type resolution at call sites
/// - **flag_returns**: builtin return types decided by a bit of a flags
///   argument, which a conditional return type cannot express
pub mod conditional;
pub mod flag_returns;
pub mod narrowing;
pub mod resolution;

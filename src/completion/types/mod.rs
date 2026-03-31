/// Type resolution sub-modules.
///
/// This group contains modules related to type resolution:
/// - **resolution**: Type-hint string to `ClassInfo` mapping (unions,
///   intersections, generics, type aliases, object shapes, property types)
/// - **narrowing**: instanceof / assert / custom type guard narrowing
/// - **conditional**: PHPStan conditional return type resolution at call sites
pub mod conditional;
pub(crate) mod narrowing;
pub(crate) mod resolution;

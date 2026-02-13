/// Goto definition support.
///
/// This module contains the logic for resolving "go to definition" requests,
/// allowing users to jump from a class/interface/trait/enum name reference
/// to its definition in the source code.
///
/// - [`resolve`]: Word extraction, name resolution, and definition location lookup.
mod resolve;

//! Source analysis sub-modules.
//!
//! This group contains modules for source-text scanning and position
//! detection utilities:
//! - **comment_position**: Comment, docblock, and string position detection
//!   (`is_inside_docblock`, `is_inside_non_doc_comment`, `classify_string_context`)
//! - **helpers**: Source-text scanning helpers (closure return types,
//!   first-class callable resolution, `new` expression parsing, array access)
//! - **throws_analysis**: Throws analysis pipeline (throw scanning, catch-block
//!   filtering, uncaught detection, method `@throws` / return-type lookup)

pub mod comment_position;
pub(crate) mod helpers;
pub(crate) mod throws_analysis;

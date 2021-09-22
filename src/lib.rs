//! A CLI program for managing files across multiple devices.
//!
//! You can think of `hoard` as a dotfiles management tool, though its intended
//! use extends beyond that. `hoard` can be used for backing up and restoring
//! any kind of file from/to any location on the filesystem. In fact, the
//! original purpose behind writing it was to synchronize save files for games
//! that don't support cloud saves.
//!
//! # Terminology
//!
//! The following terms have special meanings when talking about `hoard`.
//!
//! - [`Hoard`](crate::config::builder::hoard::Hoard): A collection at least one
//!   [`Pile`](crate::config::builder::hoard::Pile).
//! - [`Pile`](crate::config::builder::hoard::Pile): A single file or directory
//!   in a [`Hoard`](crate::config::builder::hoard::Hoard).
//! - [`Environment`](crate::config::builder::environment::Environment): A
//!   combination of conditions that can be used to determine where to find
//!   files in a [`Pile`](crate::config::builder::hoard::Pile).

#![deny(
    clippy::all,
    clippy::complexity,
    clippy::correctness,
    clippy::pedantic,
    clippy::perf,
    clippy::restriction,
    clippy::style
)]
#![allow(
    clippy::string_add,
    clippy::blanket_clippy_restriction_lints,
    clippy::filetype_is_file,
    clippy::create_dir,
    clippy::else_if_without_else,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::exit,
    clippy::implicit_return,
    clippy::indexing_slicing,
    clippy::integer_arithmetic,
    clippy::integer_division,
    clippy::missing_docs_in_private_items,
    clippy::missing_errors_doc,
    clippy::missing_inline_in_public_items,
    clippy::module_name_repetitions,
    clippy::pattern_type_mismatch,
    clippy::shadow_reuse,

    // Need to be fixed
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic_in_result_fn,
    clippy::unreachable,
    clippy::unwrap_in_result,
    clippy::expect_fun_call,
    clippy::unimplemented,
)]
#![deny(
    absolute_paths_not_starting_with_crate,
    anonymous_parameters,
    bad_style,
    const_err,
    dead_code,
    ellipsis_inclusive_range_patterns,
    exported_private_dependencies,
    ill_formed_attribute_input,
    improper_ctypes,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse, // May have false positives
    missing_abi,
    missing_debug_implementations, // can affect compile time/code size
    missing_docs,
    // missing_doc_code_examples,
    no_mangle_generic_items,
    non_shorthand_field_patterns,
    noop_method_call,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    pointer_structural_match,
    private_in_public,
    pub_use_of_private_extern_crate,
    semicolon_in_expressions_from_macros,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unaligned_references,
    unconditional_recursion,
    unreachable_pub,
    unused,
    unused_allocation,
    unused_comparisons,
    // unused_crate_dependencies,
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_parens,
    unused_qualifications,
    variant_size_differences,
    while_true
)]
// clippy::blacklisted_name,
#![cfg_attr(
    any(test),
    allow(
        clippy::expect_fun_call,
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        clippy::wildcard_enum_match_arm,
    )
)]

pub use config::Config;

pub mod checkers;
pub mod combinator;
pub mod command;
pub mod config;
pub mod env_vars;
#[macro_use]
pub mod macros;
pub mod utils;

/// The default file name of the configuration file.
pub const CONFIG_FILE_NAME: &str = "config.yml";

/// The name of the directory containing the backed up hoards.
pub const HOARDS_DIR_SLUG: &str = "hoards";

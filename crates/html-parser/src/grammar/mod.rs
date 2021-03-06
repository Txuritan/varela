#![allow(clippy::upper_case_acronyms)]

use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar/rules.pest"]
pub struct Grammar;

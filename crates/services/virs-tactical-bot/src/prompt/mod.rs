mod ai_generator;
mod loader;
mod render;
mod template;
mod validator;
mod writer;

pub use ai_generator::{generate_prompt, GenerateRequest, GenerateResult};
pub use loader::PromptLoader;
pub use render::{format_bars_outside, render, RenderContext};
pub use template::{MetaFile, PromptSource, PromptTemplate};
pub use validator::validate;
pub use writer::{delete_template, save_template};

#[cfg(test)]
mod ai_generator_tests;
#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod validator_tests;
#[cfg(test)]
mod writer_tests;

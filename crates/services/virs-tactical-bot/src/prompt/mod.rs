mod ai_generator;

pub use ai_generator::{generate_prompt, GenerateRequest, GenerateResult};

#[cfg(test)]
mod ai_generator_tests;

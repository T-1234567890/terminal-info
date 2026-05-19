use terminal_info::ai::chat::ProviderKind;
use terminal_info::ai::cli_chat::{AiPromptOptions, complete_prompt};

use super::render::render_markdown;
use super::symbolic::SolveOutput;
use super::{Audience, ExplainStyle};
use crate::math::ast::MathDocument;

pub fn deterministic_explanation(
    document: &MathDocument,
    output: &SolveOutput,
    audience: Audience,
    style: ExplainStyle,
) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "Math explanation for {} audience in {} style.\n\n",
        audience.label(),
        style.label()
    ));
    text.push_str(&render_markdown(document, output, true));
    text
}

pub fn ai_explanation(
    document: &MathDocument,
    output: &SolveOutput,
    audience: Audience,
    style: ExplainStyle,
    provider: Option<ProviderKind>,
    stream_to_stdout: bool,
) -> Result<String, String> {
    let system = "You explain deterministic symbolic math output from Terminal Info. Do not solve independently. Do not change answers. Do not invent missing steps. Call out unsupported operations as unsupported.".to_string();
    let prompt = format!(
        "Audience: {}\nStyle: {}\n\nExplain this deterministic math report without changing any result:\n\n{}",
        audience.label(),
        style.label(),
        render_markdown(document, output, true)
    );
    complete_prompt(AiPromptOptions {
        provider,
        model: None,
        system: Some(system),
        prompt,
        stream_to_stdout,
    })
}

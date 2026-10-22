//! Prefix size report: what every LLM call pays before the first user token
//! (system prompt + serialized tool definitions), per agent and tool scope.
//! Run with `cargo test --lib size_report -- --ignored --nocapture`.

use std::sync::{Arc, Mutex};

use rig_agent::tool::Tool;

use crate::agents::{
    analyst_agent, block_generator_agent, closure_feedback_grader_agent, lexical_assistant_agent, notebook_gate_grader_agent,
    pedagogical_critic_agent, roadmap_agent,
};
use crate::prompt_encoding::TOON_HINT;
use crate::tools::{
    ConfirmSyllabusPlanTool, DiagnosticToolScope, GradeClosureSubmissionTool, GradeGateSubmissionTool, PresentDiagnosticBatteryTool,
    ProposeCapstoneProjectTool, ProposeSyllabusPlanTool, PublishNotebookBlockTool, RoadmapCapture, SubmitBlockAuditTool,
    SubmitDiagnosticAssessmentTool,
};

/// Compact JSON of one tool exactly as providers receive it (name, description, parameters).
fn tool_json<T: Tool>(tool: &T) -> String {
    serde_json::json!({ "name": T::NAME, "description": tool.description(), "parameters": tool.parameters() }).to_string()
}

fn roadmap_tools(scope: DiagnosticToolScope) -> Vec<String> {
    let cap = Arc::new(Mutex::new(RoadmapCapture::default()));
    let mut out = Vec::new();
    if scope.allows_assessment() {
        out.push(tool_json(&SubmitDiagnosticAssessmentTool(cap.clone())));
    }
    if scope.allows_battery() {
        out.push(tool_json(&PresentDiagnosticBatteryTool(cap.clone())));
    }
    if scope.allows_propose() {
        out.push(tool_json(&ProposeSyllabusPlanTool(cap.clone())));
        out.push(tool_json(&ProposeCapstoneProjectTool(cap.clone())));
    }
    if scope.allows_confirm() {
        out.push(tool_json(&ConfirmSyllabusPlanTool(cap)));
    }
    out
}

fn est_tokens(bytes: usize) -> usize {
    bytes / 4
}

/// `(label, system prompt, tool JSON, TOON hint included in the user input)`.
fn rows() -> Vec<(String, String, Vec<String>, bool)> {
    let mut rows = vec![
        ("notebook_generator".to_string(), block_generator_agent::definition().system_prompt, vec![tool_json(&PublishNotebookBlockTool(Default::default()))], true),
        ("pedagogical_critic".to_string(), pedagogical_critic_agent::definition().system_prompt, vec![tool_json(&SubmitBlockAuditTool(Default::default()))], true),
        ("notebook_gate_grader".to_string(), notebook_gate_grader_agent::definition().system_prompt, vec![tool_json(&GradeGateSubmissionTool(Default::default()))], true),
        (
            "closure_feedback_grader".to_string(),
            closure_feedback_grader_agent::definition().system_prompt,
            vec![tool_json(&GradeClosureSubmissionTool(Default::default()))],
            true,
        ),
        ("lexical_assistant".to_string(), lexical_assistant_agent::definition().system_prompt, vec![], true),
        (
            "diagnostic_analyst [Assessment]".to_string(),
            analyst_agent::definition().system_prompt,
            roadmap_tools(DiagnosticToolScope::Assessment),
            true,
        ),
    ];
    for (name, scope) in [
        ("All", DiagnosticToolScope::All),
        ("Assessment", DiagnosticToolScope::Assessment),
        ("Battery", DiagnosticToolScope::Battery),
        ("Propose", DiagnosticToolScope::Propose),
    ] {
        rows.push((format!("roadmap [{name}]"), roadmap_agent::definition().system_prompt, roadmap_tools(scope), true));
    }
    rows
}

#[test]
#[ignore = "prints a report; run with --ignored --nocapture"]
fn size_report() {
    println!("\n{:<34} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}", "agent [scope]", "sys B", "sys tok", "tools B", "tools tok", "TOON B", "prefix tok");
    for (label, sys, tools, toon) in rows() {
        let tools_b: usize = tools.iter().map(String::len).sum();
        let hint_b = if toon { TOON_HINT.len() } else { 0 };
        let prefix = est_tokens(sys.len()) + est_tokens(tools_b);
        println!(
            "{:<34} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
            label,
            sys.len(),
            est_tokens(sys.len()),
            tools_b,
            est_tokens(tools_b),
            hint_b,
            prefix
        );
    }
    println!("(prefix tok = est(system) + est(tools), bytes/4; TOON B is per user input, counted outside the prefix)");
}

//! Prefix size report: what every LLM call pays before the first user token
//! (system prompt + serialized tool definitions), per agent and tool scope.
//! Run with `cargo test --lib size_report -- --ignored --nocapture`.

use std::sync::{Arc, Mutex};

use rig_agent::tool::Tool;

use crate::agents::{
    analyst_agent, block_generator_agent, closure_feedback_grader_agent, lexical_assistant_agent, notebook_gate_grader_agent,
    pedagogical_critic_agent, roadmap_agent,
};
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

/// `(label, system prompt, tool JSON)`. The TOON hint (since T5) is part of
/// each TOON-consuming agent's system prompt, so it is counted in `sys B`.
fn rows() -> Vec<(String, String, Vec<String>)> {
    let mut rows = vec![
        ("notebook_generator".to_string(), block_generator_agent::definition().system_prompt, vec![tool_json(&PublishNotebookBlockTool(Default::default()))]),
        ("pedagogical_critic".to_string(), pedagogical_critic_agent::definition().system_prompt, vec![tool_json(&SubmitBlockAuditTool(Default::default()))]),
        ("notebook_gate_grader".to_string(), notebook_gate_grader_agent::definition().system_prompt, vec![tool_json(&GradeGateSubmissionTool(Default::default()))]),
        (
            "closure_feedback_grader".to_string(),
            closure_feedback_grader_agent::definition().system_prompt,
            vec![tool_json(&GradeClosureSubmissionTool(Default::default()))],
        ),
        ("lexical_assistant".to_string(), lexical_assistant_agent::definition().system_prompt, vec![]),
        ("diagnostic_analyst [Assessment]".to_string(), analyst_agent::definition().system_prompt, roadmap_tools(DiagnosticToolScope::Assessment)),
    ];
    for (name, scope) in [
        ("All", DiagnosticToolScope::All),
        ("Assessment", DiagnosticToolScope::Assessment),
        ("Battery", DiagnosticToolScope::Battery),
        ("Propose", DiagnosticToolScope::Propose),
    ] {
        rows.push((format!("roadmap [{name}]"), roadmap_agent::definition().system_prompt, roadmap_tools(scope)));
    }
    rows
}

#[test]
#[ignore = "prints a report; run with --ignored --nocapture"]
fn size_report() {
    println!("\n{:<34} {:>9} {:>9} {:>9} {:>9} {:>10}", "agent [scope]", "sys B", "sys tok", "tools B", "tools tok", "prefix tok");
    for (label, sys, tools) in rows() {
        let tools_b: usize = tools.iter().map(String::len).sum();
        let prefix = est_tokens(sys.len()) + est_tokens(tools_b);
        println!("{:<34} {:>9} {:>9} {:>9} {:>9} {:>10}", label, sys.len(), est_tokens(sys.len()), tools_b, est_tokens(tools_b), prefix);
    }
    println!("(prefix tok = est(system) + est(tools), bytes/4; baseline had a separate +165 B TOON hint in every user input)");
}

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;

use super::*;
use crate::agents::AgentRegistry;
use crate::domain::notebook::DiagnosticQuestion;
use crate::domain::roadmap::{CapstoneProjectArgs, ConfirmSyllabusPlanArgs, ProposeSyllabusPlanArgs};
use crate::orchestration::PromptRunner;
use crate::providers::factory::PromptOutput;
use crate::domain::learning_context::ContextQuestionsArgs;
use crate::tools::{DiagnosticToolScope, ResearchStep, SharedResearchCapture, SharedRoadmapCapture};

mod followups;
mod form;
mod gates;
mod harness;
mod rejections;
mod retries;
mod sessions;

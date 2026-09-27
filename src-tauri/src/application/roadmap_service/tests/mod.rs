use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;

use super::*;
use crate::agents::AgentRegistry;
use crate::domain::roadmap::{ConfirmSyllabusPlanArgs, ProposeSyllabusPlanArgs};
use crate::orchestration::PromptRunner;
use crate::providers::factory::PromptOutput;
use crate::tools::{DiagnosticToolScope, SharedRoadmapCapture};

mod followups;
mod gates;
mod harness;
mod rejections;
mod retries;
mod sessions;

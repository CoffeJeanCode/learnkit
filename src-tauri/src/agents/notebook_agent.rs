//! Compatibility shim: the notebook block generator now lives in
//! [`block_generator_agent`](crate::agents::block_generator_agent) alongside
//! its sibling execution agents. Re-exported here so existing imports
//! (`crate::agents::notebook_agent::NOTEBOOK_AGENT_ID`) keep resolving.

pub use crate::agents::block_generator_agent::{NOTEBOOK_AGENT_ID, definition};

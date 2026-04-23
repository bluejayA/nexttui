//! Runtime cloud/project context switching primitives.
//!
//! BL-P2-031 Unit 1: Foundation Types.
//!
//! This module hosts the shared types and small utilities used across the
//! switch orchestration. It has no behavioural dependencies on other modules
//! so that downstream units (concurrency infra, session port, switcher) can
//! build on top of a stable vocabulary.

pub mod action_channel;
pub mod cancellation;
pub mod capabilities;
pub mod config_cloud_directory;
pub mod epoch;
pub mod error;
pub mod history;
pub mod resolver;
pub mod state_machine;
pub mod static_project_directory;
pub mod switcher;
pub mod types;
pub mod versioned;

pub use action_channel::{ActionReceiver, ActionSender, test_action_channel};
pub use cancellation::CancellationRegistry;
pub use capabilities::{AuthMethod, KeystoneCapabilities, KeystoneVersion};
pub use config_cloud_directory::ConfigCloudDirectory;
pub use epoch::{ContextEpoch, Epoch};
pub use error::SwitchError;
pub use history::ContextHistoryStore;
pub use resolver::{CloudDirectory, ContextTargetResolver, ProjectCandidate, ProjectDirectoryPort};
pub use state_machine::{SwitchStateMachine, SwitchStateView};
#[cfg(test)]
pub use static_project_directory::StaticProjectDirectory;
pub use switcher::ContextSwitcher;
pub use types::{ContextRequest, ContextSnapshot, ContextTarget, SessionHandle};
pub use versioned::VersionedEvent;

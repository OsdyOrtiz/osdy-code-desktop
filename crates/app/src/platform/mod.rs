//! OS integration: the single-instance guard, the agents' command shims,
//! the user's `ocd` command, the Dock/window icons, the installed code
//! editors and web browsers, the power state and whether a terminal's
//! shell is idle.

pub mod browsers;
pub mod font_metadata;
pub mod gsd;
pub mod icons;
pub mod ides;
pub mod instance;
pub mod power;
pub mod shell_idle;
pub mod shims;

//! Store actions: writing `profiles.json`, Save, Use in this session, Make
//! default, the repository pin, Publish, Export and Import, and the name and
//! leave flows. On the RPC path the same actions are whole-store `set_config`
//! writes guarded by the last `writeRevision`, and Publish is the `pins`
//! store's `repo` layer. Make default and Use in this session are Gentle
//! Shell's `apply_profile` when it advertises it; otherwise Make default
//! stays on the files. Clearing the session binding is its
//! `clear_profile_binding` where advertised, else `/gentle:profiles clear`.

use gentle_core::profiles::{
    PROFILE_EXPORT_FILE, PinRead, ProfileExport, ProfilesFile, Routing, normalize_routing,
    parse_model_ref, parse_profile_export, parse_profiles, serialize_profiles,
};
use gpui_kit::*;
use pi_rpc::desktop_rpc::{
    ApplyProfileResult, ClearBindingStatus, ConfigStore, ProfileApplyMode, ProfileNote,
    ProfileNoteLevel,
};

use super::apply::{make_default, plan_make_default};
use super::env::{ApplyAnswer, ConfigOutcome, ProfileApply};
use super::files::{
    fallbacks_write, files_job_start, follow_fallbacks, profiles_write, read_orchestrator_settings,
    write_atomic,
};
use super::pins::{
    clear_pin, follow_renamed_pin, local_pin_write, read_pin_status, renamed_pin_write,
    repo_pin_write, write_pin,
};
use super::{
    Backend, Confirm, Leave, NameMode, ProfileText, ProfilesEvent, ProfilesView, Tone, profile_msg,
};
use crate::shared::i18n::t;

/// The committed declaration, as the RPC path names it.
const DECLARATION: &str = ".pi/gentle-ai/profile.json";

/// The stores Make default may have written through `apply_profile`.
const APPLIED_STORES: [ConfigStore; 5] = [
    ConfigStore::Profiles,
    ConfigStore::Pins,
    ConfigStore::Models,
    ConfigStore::Subagents,
    ConfigStore::ManagedAssets,
];

/// Gentle Shell's notes as one status line, in order.
fn notes_text(notes: &[ProfileNote]) -> String {
    notes
        .iter()
        .map(|note| note.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl ProfilesView {
    /// Write `next` to profiles.json (or replace the `profiles` store over
    /// RPC), then adopt it.
    fn write_store(
        &mut self,
        next: ProfilesFile,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        if self.is_rpc() {
            let write = match profiles_write(&next) {
                Ok(write) => write,
                Err(error) => {
                    return self.say(profile_msg!("Not saved: {error}", error = error), Tone::Bad);
                }
            };
            self.rpc_write(write, cx, move |this, result, cx| {
                // The Shell's normalized store is the truth; the draft it was
                // built from stands in only if that cannot be read back.
                this.file = result
                    .state
                    .value
                    .as_ref()
                    .and_then(|value| parse_profiles(&value.to_string()))
                    .map_or(next, |(file, _)| file);
                done(this, cx);
                // A pin's evaluation depends on which profiles exist.
                this.reread(&[ConfigStore::Pins], cx);
            });
            return;
        }
        let Some(env) = self.env.clone() else { return };
        let text = serialize_profiles(&next);
        self.run_io(
            cx,
            move || write_atomic(&env.profiles_path(), &text).map_err(|e| e.to_string()),
            move |this, result, cx| match result {
                Ok(()) => {
                    this.file = next;
                    cx.emit(ProfilesEvent::Changed);
                    done(this, cx);
                }
                Err(error) => this.say(
                    profile_msg!("profiles.json was not written: {error}", error = error),
                    Tone::Bad,
                ),
            },
        );
    }

    /// The draft (routing, orchestrator mode and Controlled Engineering) as
    /// the store would hold it
    /// (create or update), or `None` when it already matches.
    fn saved_draft(&self) -> Result<Option<ProfilesFile>, ProfileText> {
        let Some(name) = &self.selected else {
            return Ok(None);
        };
        let normalized = normalize_routing(&self.draft.to_json()).unwrap_or_default();
        let stored = self
            .file
            .get(name)
            .map(|r| normalize_routing(&r.to_json()).unwrap_or_default());
        let workflow_saved = self.file.orchestrator_mode(name) == self.draft_mode
            && self.file.controlled_engineering(name) == self.draft_controlled;
        if stored.as_ref() == Some(&normalized) && workflow_saved {
            return Ok(None);
        }
        let next = if self.file.has(name) {
            self.file.update(name, self.draft.clone())
        } else {
            self.file.create(name, self.draft.clone())
        };
        next.and_then(|next| next.with_orchestrator_mode(name, self.draft_mode))
            .and_then(|next| next.with_controlled_engineering(name, self.draft_controlled))
            .map(Some)
            .map_err(|error| ProfileText::store_error(error.into_parts()))
    }

    /// Save pending edits, then continue with `then` (told whether it saved).
    pub(super) fn save_then(
        &mut self,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, bool, &mut Context<Self>) + 'static,
    ) {
        match self.saved_draft() {
            Err(error) => self.say(profile_msg!("Not saved: {error}", error = error), Tone::Bad),
            Ok(None) => then(self, false, cx),
            Ok(Some(next)) => self.write_store(next, cx, move |this, cx| {
                this.stored = this.draft.clone();
                this.stored_mode = this.draft_mode;
                this.stored_controlled = this.draft_controlled;
                then(this, true, cx);
            }),
        }
        cx.notify();
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        if self.change_count() == 0 {
            self.say("Nothing to save.", Tone::Info);
            cx.notify();
            return;
        }
        self.save_then(cx, |this, saved, _| {
            let name = this.selected.clone().unwrap_or_default();
            this.say(
                if saved {
                    profile_msg!("Saved \"{name}\".", name = name)
                } else {
                    "Nothing to save.".into()
                },
                Tone::Good,
            );
        });
    }

    pub(super) fn use_in_session(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.shell_applies(cx) {
            self.save_then(cx, move |this, saved, cx| {
                if saved {
                    this.say(
                        profile_msg!("Saved \"{name}\".", name = name.clone()),
                        Tone::Good,
                    );
                }
                this.shell_apply(ProfileApply::new(name, ProfileApplyMode::Session), cx);
            });
            return;
        }
        self.save_then(cx, move |this, saved, cx| {
            let orchestrator = this.draft.orchestrator();
            let model = orchestrator
                .and_then(|e| e.model.as_deref())
                .and_then(parse_model_ref);
            let thinking = model
                .as_ref()
                .and_then(|_| orchestrator.and_then(|e| e.thinking.clone()));
            let mut lines = Vec::new();
            if saved {
                lines.push(profile_msg!("Saved \"{name}\".", name = name.clone()));
            }
            lines.push(profile_msg!(
                "Using \"{name}\" in this session.",
                name = name.clone()
            ));
            lines.push(match &model {
                Some((provider, id)) => profile_msg!(
                    "Switching this session to {model}.",
                    model = format!("{provider}/{id}")
                ),
                None => "The orchestrator is left untouched.".into(),
            });
            this.say(ProfileText::Joined(lines, " "), Tone::Good);
            cx.emit(ProfilesEvent::UseInSession {
                profile: name,
                model,
                thinking,
            });
        });
    }

    pub(super) fn request_make_default(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.shell_applies(cx) {
            self.save_then(cx, move |this, saved, cx| {
                if saved {
                    this.say(
                        profile_msg!("Saved \"{name}\".", name = name.clone()),
                        Tone::Good,
                    );
                }
                this.shell_apply(ProfileApply::new(name, ProfileApplyMode::Default), cx);
            });
            return;
        }
        self.save_then(cx, move |this, saved, cx| {
            let Some(env) = this.env.clone() else { return };
            let file = this.file.clone();
            let config = file.get(&name).cloned().unwrap_or_default();
            if saved {
                this.say(
                    profile_msg!("Saved \"{name}\".", name = name.clone()),
                    Tone::Good,
                );
            }
            this.run_io(
                cx,
                move || plan_make_default(&env, &file, &name, &config),
                |this, plan, _| this.confirm = Some(Confirm::MakeDefault(plan)),
            );
        });
    }

    pub(super) fn make_default(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(env) = self.env.clone() else { return };
        // Make default writes profiles.json itself, so on the RPC path it
        // must not race the Shell's writer in either direction: it starts
        // only from the freshest store (no write open, no read owed), and
        // holds every store write until it finishes.
        let file = match &mut self.backend {
            Backend::Rpc(remote) => match files_job_start(remote) {
                Ok(file) => file,
                Err(error) => {
                    self.say(error, Tone::Info);
                    cx.notify();
                    return;
                }
            },
            _ => self.file.clone(),
        };
        self.run_io(
            cx,
            move || {
                let outcome = make_default(&env, &file, &name);
                (
                    outcome,
                    read_pin_status(&env.cwd),
                    read_orchestrator_settings(&env.settings_path()),
                )
            },
            |this, (outcome, pin, pi_default), cx| {
                if let Backend::Rpc(remote) = &mut this.backend {
                    remote.release();
                    // Make default wrote profiles.json and the pin directly:
                    // re-read them through the Shell so the next RPC write is
                    // guarded by the revisions it left, never by older ones.
                    this.reread(
                        &[
                            ConfigStore::Profiles,
                            ConfigStore::Pins,
                            ConfigStore::Models,
                            ConfigStore::Subagents,
                            ConfigStore::ManagedAssets,
                        ],
                        cx,
                    );
                } else {
                    this.pin = pin;
                }
                this.pi_default = pi_default;
                match outcome {
                    Ok(outcome) => {
                        this.file = outcome.file;
                        this.say(outcome.message, Tone::Good);
                        cx.emit(ProfilesEvent::Changed);
                        if let Some(entry) = outcome.orchestrator
                            && let Some(model) = entry.model.as_deref().and_then(parse_model_ref)
                        {
                            cx.emit(ProfilesEvent::SwitchOrchestrator {
                                model,
                                thinking: entry.thinking,
                            });
                        }
                    }
                    Err(error) => {
                        this.say(error, Tone::Bad);
                        cx.emit(ProfilesEvent::Changed);
                    }
                }
            },
        );
    }

    /// Ask Gentle Shell to apply a saved profile (`apply_profile`). Every
    /// store write waits until it answers.
    pub(super) fn shell_apply(&mut self, request: ProfileApply, cx: &mut Context<Self>) {
        let name = request.profile.clone();
        let sent = self.with_remote(cx, |remote, port| remote.apply(port, request));
        match sent {
            Some(Ok(_)) => self.say(
                profile_msg!("Applying \"{name}\" in Gentle Shell…", name = name),
                Tone::Info,
            ),
            Some(Err(failure)) => self.say(
                profile_msg!(
                    "\"{name}\" was not applied: {error}",
                    name = name,
                    error = failure.text()
                ),
                Tone::Bad,
            ),
            None => {}
        }
        self.sync_busy();
        cx.notify();
    }

    /// Gentle Shell answered an apply: ask the human with its exact dialog,
    /// or show its notes. Nothing is ever confirmed here.
    pub(super) fn apply_answered(
        &mut self,
        request: ProfileApply,
        result: ApplyProfileResult,
        cx: &mut Context<Self>,
    ) {
        let name = request.profile.clone();
        let mode = request.mode;
        match ApplyAnswer::new(&request, result) {
            ApplyAnswer::Confirm {
                title,
                message,
                retry,
            } => {
                self.status = None;
                self.confirm = Some(super::Confirm::ApplyProfile {
                    title,
                    message,
                    retry,
                });
            }
            ApplyAnswer::Done { applied, notes } => {
                let text = notes_text(&notes);
                if applied {
                    let text = if text.is_empty() {
                        profile_msg!("Applied \"{name}\".", name = name)
                    } else {
                        text.into()
                    };
                    self.say(text, Tone::Good);
                    cx.emit(ProfilesEvent::Changed);
                    if mode == ProfileApplyMode::Default {
                        self.refresh_after_shell_apply(cx);
                    }
                } else {
                    let text = if text.is_empty() {
                        profile_msg!("Gentle Shell did not apply \"{name}\".", name = name)
                    } else {
                        text.into()
                    };
                    let failed = notes
                        .iter()
                        .any(|note| note.level != ProfileNoteLevel::Info);
                    self.say(text, if failed { Tone::Bad } else { Tone::Info });
                    // A rollback may have restored part of what was written.
                    if mode == ProfileApplyMode::Default {
                        self.refresh_after_shell_apply(cx);
                    }
                }
            }
        }
        cx.notify();
    }

    /// Gentle Shell's Make default wrote the stores and Pi's settings: read
    /// them again, so the screen and the next write follow what it left.
    fn refresh_after_shell_apply(&mut self, cx: &mut Context<Self>) {
        self.reread(&APPLIED_STORES, cx);
        self.read_agents(false, cx);
    }

    pub(super) fn toggle_repo_pin(&mut self, cx: &mut Context<Self>) {
        let (Some(name), Some(env)) = (self.selected.clone(), self.env.clone()) else {
            return;
        };
        if self.is_rpc() {
            self.save_then(cx, move |this, _, cx| {
                let (write, pinned) = match local_pin_write(this.pin.as_ref(), &name) {
                    Ok(write) => write,
                    Err(error) => return this.say(error, Tone::Bad),
                };
                let message = if pinned {
                    "Removed the clone pin. This clone falls back to the repository declaration, then to the global routing.".into()
                } else {
                    profile_msg!("Pinned \"{name}\" for this clone. Subagent launches here resolve it; the orchestrator and other repositories keep their global routing.", name = name.clone())
                };
                this.rpc_write(write, cx, move |this, _, _| this.say(message, Tone::Good));
            });
            return;
        }
        self.save_then(cx, move |this, _, cx| {
            this.run_io(
                cx,
                move || {
                    let Some(status) = read_pin_status(&env.cwd) else {
                        return (Err("This folder is not inside a Git worktree, so nothing was pinned.".into()), None);
                    };
                    let pinned = status.local == PinRead::Valid(name.clone());
                    let result = if pinned {
                        clear_pin(&status.local_path).map_err(|e| ProfileText::from(e.to_string())).map(|_| {
                            profile_msg!("Removed the local pin from {path}. This clone falls back to the repository declaration, then to the global routing.", path = status.local_path.display().to_string())
                        })
                    } else {
                        write_pin(&status.local_path, &name).map(|_| {
                            profile_msg!("Pinned \"{name}\" for this clone in {path}. Subagent launches here resolve it; the orchestrator and other repositories keep their global routing.", name = name, path = status.local_path.display().to_string())
                        })
                    };
                    (result, read_pin_status(&env.cwd))
                },
                |this, (result, pin), cx| {
                    match result {
                        Ok(message) => {
                            this.pin = pin;
                            this.say(message, Tone::Good);
                            cx.emit(ProfilesEvent::Changed);
                        }
                        Err(error) => this.say(error, Tone::Bad),
                    }
                },
            );
        });
    }

    pub(super) fn publish(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.is_rpc() {
            self.save_then(cx, move |this, _, cx| {
                let (write, declared) = match repo_pin_write(this.pin.as_ref(), &name) {
                    Ok(write) => write,
                    Err(error) => return this.say(error, Tone::Bad),
                };
                this.rpc_write(write, cx, move |this, result, _| {
                    let message = if !result.changed {
                        profile_msg!("The repository declaration ({path}) already said that; nothing changed.", path = DECLARATION.to_string())
                    } else if declared {
                        profile_msg!("Removed the repository declaration ({path}). This worktree falls back to the global routing; a clone pin still outranks it.", path = DECLARATION.to_string())
                    } else {
                        profile_msg!("Declared \"{name}\" for this worktree in {path}. Subagent launches resolve it; commit the file to share the routing. A clone pin takes precedence over it.", name = name, path = DECLARATION.to_string())
                    };
                    this.say(message, Tone::Good);
                });
            });
            return;
        }
        self.save_then(cx, move |this, _, cx| {
            this.say(
                profile_msg!(
                    "Sent /gentle:profiles publish {name}. Refresh to see the declaration.",
                    name = name.clone()
                ),
                Tone::Info,
            );
            cx.emit(ProfilesEvent::RunCommand(format!(
                "/gentle:profiles publish {name}"
            )));
        });
    }

    pub(super) fn clear_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.session.clone()
            && self.is_rpc()
            && session.read(cx).binding_clear_supported()
        {
            match session.update(cx, |session, cx| session.clear_profile_binding(cx)) {
                Ok(ticket) => {
                    self.clearing = Some(ticket);
                    self.say("Clearing this session's profile binding…", Tone::Info);
                }
                Err(failure) => self.say(failure.text(), Tone::Bad),
            }
            cx.notify();
            return;
        }
        if let Some(session) = self.session.clone() {
            session.update(cx, |session, cx| session.set_session_profile(None, cx));
        }
        self.say("Sent /gentle:profiles clear.", Tone::Info);
        cx.emit(ProfilesEvent::RunCommand("/gentle:profiles clear".into()));
        cx.notify();
    }

    /// Gentle Shell answered `clear_profile_binding`. The session's binding
    /// itself follows from the tab ([`SessionView::session_profile`]).
    ///
    /// [`SessionView::session_profile`]: crate::features::session::SessionView::session_profile
    pub(super) fn binding_cleared(&mut self, outcome: ConfigOutcome) {
        let (message, tone) = match outcome {
            ConfigOutcome::Cleared(result) => match result.status {
                ClearBindingStatus::Cleared => (
                    match result.binding.profile {
                        Some(profile) => profile_msg!("Cleared the session binding. This session is back on \"{profile}\".", profile = profile),
                        None => "Cleared the session binding. This session is back on the routing without a session profile.".into(),
                    },
                    Tone::Good,
                ),
                ClearBindingStatus::Unchanged => (
                    "This session had no profile binding to clear.".into(),
                    Tone::Info,
                ),
                ClearBindingStatus::NotCleared => (
                    "Gentle Shell could not confirm the clear; run /gentle:profiles in the terminal to see why.".into(),
                    Tone::Bad,
                ),
            },
            ConfigOutcome::Failed(failure) => (
                profile_msg!("The session binding was not cleared: {error}", error = failure.text()),
                Tone::Bad,
            ),
            _ => (
                "The answer to the clear did not match it.".into(),
                Tone::Bad,
            ),
        };
        self.say(message, tone);
    }

    /// Leaving the shown profile with unsaved edits asks first.
    pub(super) fn leave(&mut self, leave: Leave, window: &mut Window, cx: &mut Context<Self>) {
        if self.change_count() > 0 {
            self.confirm = Some(Confirm::Unsaved(leave));
        } else {
            self.proceed(leave, window, cx);
        }
        cx.notify();
    }

    pub(super) fn proceed(&mut self, leave: Leave, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm = None;
        match leave {
            Leave::Switch(name) => {
                self.select(Some(name.clone()), false);
                self.say(profile_msg!("Showing \"{name}\".", name = name), Tone::Info);
            }
            Leave::Close => cx.emit(ProfilesEvent::Close),
            Leave::New => self.open_name(NameMode::New, window, cx),
            Leave::Duplicate => self.open_name(NameMode::Duplicate, window, cx),
        }
        cx.notify();
    }

    pub(super) fn open_name(
        &mut self,
        mode: NameMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.name_mode = Some(mode);
        self.name_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn submit_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mode) = self.name_mode else { return };
        let answer = self.name_input.read(cx).value().trim().to_string();
        let current = self.selected.clone().unwrap_or_default();
        let result = match mode {
            NameMode::New if answer.is_empty() => {
                Err("No profile created: no name entered.".into())
            }
            NameMode::New => self
                .file
                .create(&answer, Routing::default())
                .map(|f| (f, answer.clone()))
                .map_err(|error| ProfileText::store_error(error.into_parts())),
            NameMode::Duplicate => {
                let to = if answer.is_empty() {
                    format!("{current}-copy")
                } else {
                    answer.clone()
                };
                self.file
                    .duplicate(&current, &to)
                    .map(|f| (f, to))
                    .map_err(|error| ProfileText::store_error(error.into_parts()))
            }
            NameMode::Rename if answer.is_empty() => Err(profile_msg!(
                "Profile \"{name}\" unchanged: no new name entered.",
                name = current.clone()
            )),
            NameMode::Rename => self
                .file
                .rename(&current, &answer)
                .map(|f| (f, answer.clone()))
                .map_err(|error| ProfileText::store_error(error.into_parts())),
        };
        self.name_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        match result {
            Err(error) => self.say(error, Tone::Bad),
            Ok((next, name)) => {
                self.name_mode = None;
                let from = current.clone();
                self.write_store(next, cx, move |this, cx| {
                    match mode {
                        NameMode::Rename => {
                            let keep = this.change_count() > 0;
                            this.select(Some(name.clone()), keep);
                            this.say(profile_msg!("Profile \"{from}\" renamed to \"{name}\".", from = from.clone(), name = name.clone()), Tone::Good);
                            if this.is_rpc() {
                                this.follow_rename_rpc(&from, &name, cx);
                            } else if let Some(env) = this.env.clone() {
                                let (old, new) = (from.clone(), name.clone());
                                this.run_io(
                                    cx,
                                    move || {
                                        follow_fallbacks(&env.config_home, &old, Some(&new));
                                        let follow = follow_renamed_pin(&env.cwd, &old, &new);
                                        (follow, read_pin_status(&env.cwd))
                                    },
                                    move |this, ((followed, declared), pin), _| {
                                        this.pin = pin;
                                        let mut messages = vec![profile_msg!("Profile \"{from}\" renamed to \"{name}\".", from = from.clone(), name = name)];
                                        for path in followed {
                                            messages.push(profile_msg!("Updated the clone pin {path}.", path = path.display().to_string()));
                                        }
                                        for path in declared {
                                            messages.push(profile_msg!("The committed declaration {path} still names \"{from}\"; Publish to repo republishes it.", path = path.display().to_string(), from = from.clone()));
                                        }
                                        this.say(ProfileText::Joined(messages, " "), Tone::Good);
                                    },
                                );
                            }
                        }
                        NameMode::New | NameMode::Duplicate => {
                            this.select(Some(name.clone()), false);
                            let message = if mode == NameMode::New {
                                profile_msg!("Profile \"{name}\" created.", name = name)
                            } else {
                                profile_msg!("Profile \"{name}\" duplicated from \"{from}\".", name = name, from = from)
                            };
                            this.say(message, Tone::Good);
                        }
                    }
                });
            }
        }
        cx.notify();
    }

    pub(super) fn request_delete(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected.clone() else {
            return;
        };
        if self.file.active.as_deref() == Some(name.as_str()) {
            self.say(
                profile_msg!(
                    "Profile {name} is active. Apply another profile before deleting it.",
                    name = name
                ),
                Tone::Bad,
            );
        } else if let Some(status) = &self.pin
            && let layers = [
                ("local pin", &status.local, &status.local_path),
                ("repo pin", &status.repo, &status.repo_path),
            ]
            .into_iter()
            .filter(|(_, read, _)| **read == PinRead::Valid(name.clone()))
            .map(|(layer, _, path)| {
                profile_msg!(
                    "{pin} at {path}",
                    pin = layer,
                    path = path.display().to_string()
                )
            })
            .collect::<Vec<_>>()
            && !layers.is_empty()
        {
            self.say(
                profile_msg!("Profile \"{name}\" is pinned for this repository ({layers}). Remove the pin first (Use in this repo for the clone pin, Publish to repo for the declaration), then delete it.", name = name, layers = ProfileText::Joined(layers, " and ")),
                Tone::Bad,
            );
        } else {
            self.confirm = Some(Confirm::Delete(name));
        }
        cx.notify();
    }

    pub(super) fn delete(&mut self, name: String, cx: &mut Context<Self>) {
        self.confirm = None;
        match self.file.delete(&name) {
            Err(error) => self.say(
                profile_msg!(
                    "Profile not deleted: {error}",
                    error = ProfileText::store_error(error.into_parts())
                ),
                Tone::Bad,
            ),
            Ok(next) => self.write_store(next, cx, move |this, cx| {
                let first = this.file.names().next().map(str::to_string);
                this.select(first, false);
                this.say(
                    profile_msg!("Deleted \"{name}\".", name = name.clone()),
                    Tone::Good,
                );
                if this.is_rpc() {
                    this.follow_fallbacks_rpc(&name, None, cx);
                } else if let Some(env) = this.env.clone() {
                    this.run_io(
                        cx,
                        move || follow_fallbacks(&env.config_home, &name, None),
                        |_, _, _| {},
                    );
                }
            }),
        }
        cx.notify();
    }

    /// Save pending edits, then write the selected profile as an export
    /// (Gentle Shell's `profiles.export.json` format) where the user picks.
    pub(super) fn export_profile(&mut self, cx: &mut Context<Self>) {
        self.save_then(cx, |this, _, cx| this.export_saved(cx));
    }

    fn export_saved(&mut self, cx: &mut Context<Self>) {
        let (Some(name), Some(env)) = (self.selected.clone(), self.env.clone()) else {
            return;
        };
        let Some(config) = self.file.get(&name) else {
            return;
        };
        let text = match ProfilesFile::export(&name, config) {
            Ok(text) => text,
            Err(error) => {
                self.say(
                    profile_msg!(
                        "Profile not exported: {error}",
                        error = ProfileText::store_error(error.into_parts())
                    ),
                    Tone::Bad,
                );
                cx.notify();
                return;
            }
        };
        // Gentle Shell's own export path by default, so `/gentle:profiles
        // import` in a terminal finds it.
        let picked = cx.prompt_for_new_path(&env.config_home, Some(PROFILE_EXPORT_FILE));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.run_io(
                    cx,
                    move || {
                        write_atomic(&path, &text)
                            .map(|()| path)
                            .map_err(|e| e.to_string())
                    },
                    move |this, result, _| match result {
                        Ok(path) => this.say(
                            profile_msg!(
                                "Exported \"{name}\" to {path}.",
                                name = name,
                                path = path.display().to_string()
                            ),
                            Tone::Good,
                        ),
                        Err(error) => this.say(
                            profile_msg!("Profile not exported: {error}", error = error),
                            Tone::Bad,
                        ),
                    },
                );
            });
        })
        .detach();
    }

    /// Save pending edits, then read a profile export the user picks and
    /// add it (or, after asking, replace the profile of the same name).
    pub(super) fn import_profile(&mut self, cx: &mut Context<Self>) {
        self.save_then(cx, |_, _, cx| {
            let picked = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(t("Import").into()),
            });
            cx.spawn(async move |this, cx| {
                let Ok(Ok(Some(paths))) = picked.await else {
                    return;
                };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    this.run_io(
                        cx,
                        move || {
                            let text = std::fs::read_to_string(&path);
                            (path, text.map_err(|e| e.to_string()))
                        },
                        |this, (path, text), cx| match text.map(|t| parse_profile_export(&t)) {
                            Err(error) => this.say(
                                profile_msg!("Could not read {path}: {error}", path = path.display().to_string(), error = error),
                                Tone::Bad,
                            ),
                            Ok(None) => this.say(
                                profile_msg!("{path} is not a profile export (gentle-pi.agent_model_profile).", path = path.display().to_string()),
                                Tone::Bad,
                            ),
                            Ok(Some(export)) if this.file.has(&export.name) => {
                                this.confirm = Some(Confirm::ReplaceImport(export));
                            }
                            Ok(Some(export)) => this.add_import(export, cx),
                        },
                    );
                });
            })
            .detach();
        });
    }

    /// Write an imported profile into the store and show it.
    pub(super) fn add_import(&mut self, export: ProfileExport, cx: &mut Context<Self>) {
        self.confirm = None;
        let ProfileExport {
            name,
            config,
            dropped,
        } = export;
        let routes = config.iter().count();
        let next = if self.file.has(&name) {
            self.file.update(&name, config)
        } else {
            self.file.create(&name, config)
        };
        match next {
            Err(error) => self.say(
                profile_msg!(
                    "Profile not imported: {error}",
                    error = ProfileText::store_error(error.into_parts())
                ),
                Tone::Bad,
            ),
            Ok(next) => self.write_store(next, cx, move |this, _| {
                this.select(Some(name.clone()), false);
                let mut messages = vec![profile_msg!(
                    if routes == 1 {
                        "Imported \"{name}\" ({count} route)."
                    } else {
                        "Imported \"{name}\" ({count} routes)."
                    },
                    name = name,
                    count = routes.to_string()
                )];
                if !dropped.is_empty() {
                    messages.push(profile_msg!(
                        "Left out invalid entries: {entries}.",
                        entries = dropped.join(", ")
                    ));
                }
                this.say(ProfileText::Joined(messages, " "), Tone::Good);
            }),
        }
        cx.notify();
    }

    /// A rename over RPC: the fallbacks and the clone pin follow it; the
    /// committed declaration is only reported, like the direct-file path.
    fn follow_rename_rpc(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        self.follow_fallbacks_rpc(from, Some(to), cx);
        let mut messages = vec![profile_msg!(
            "Profile \"{from}\" renamed to \"{name}\".",
            from = from.to_string(),
            name = to.to_string()
        )];
        if self.pin.as_ref().map(|pin| &pin.repo) == Some(&PinRead::Valid(from.to_string())) {
            messages.push(profile_msg!("The committed declaration {path} still names \"{from}\"; Publish to repo republishes it.", path = DECLARATION.to_string(), from = from.to_string()));
        }
        self.say(ProfileText::Joined(messages.clone(), " "), Tone::Good);
        if let Some(write) = renamed_pin_write(self.pin.as_ref(), from, to) {
            messages.push("Updated the clone pin.".into());
            self.rpc_write(write, cx, move |this, _, _| {
                this.say(ProfileText::Joined(messages, " "), Tone::Good);
            });
        }
    }

    /// `profile_fallbacks` follows a rename or delete over RPC, from the
    /// store as last read; a missing or invalid store is left alone.
    fn follow_fallbacks_rpc(&mut self, from: &str, to: Option<&str>, cx: &mut Context<Self>) {
        let write = self
            .remote()
            .and_then(|remote| remote.state(ConfigStore::ProfileFallbacks))
            .and_then(|state| fallbacks_write(state, from, to));
        match write {
            None => {}
            Some(Err(error)) => self.say(
                profile_msg!(
                    "The profile fallbacks were not updated: {error}",
                    error = error
                ),
                Tone::Bad,
            ),
            Some(Ok(write)) => {
                self.rpc_write(write, cx, |_, _, _| {});
            }
        }
    }

    /// `proceed` for paths that run after async I/O and have no window.
    pub(super) fn proceed_without_window(&mut self, leave: Leave, cx: &mut Context<Self>) {
        match leave {
            Leave::Switch(name) => {
                self.select(Some(name.clone()), false);
                self.say(
                    profile_msg!("Saved. Showing \"{name}\".", name = name),
                    Tone::Info,
                );
            }
            Leave::Close => cx.emit(ProfilesEvent::Close),
            Leave::New => self.name_mode = Some(NameMode::New),
            Leave::Duplicate => self.name_mode = Some(NameMode::Duplicate),
        }
        cx.notify();
    }
}

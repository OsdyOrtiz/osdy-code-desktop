//! What the composer sends: `!` shell commands, built-ins and natively shown
//! commands run here, prompts with their image attachments and long pastes.

use crate::shared::i18n::t;
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gpui_kit::component::input::{InlineToken, InputContent, Position};
use gpui_kit::*;
use serde_json::json;

use super::{SessionEvent, SessionView, attachments, builtins};
use gentle_core::commands::PiBuiltin;

/// What a `!` shell command still carrying a file or an image is told: no
/// shell answer would show the attachment.
const SHELL_ATTACHMENTS: &str =
    "Shell commands cannot carry attachments. Remove them or drop the leading !.";

/// What a model that takes no image input is told when images wait beside the
/// text.
fn image_warning(model: &str) -> String {
    t("{model} does not accept images. Switch to a model with image input, or remove the attachments.").replace("{model}", model)
}

impl SessionView {
    /// Enter, and the strip's send button: what the composer would send. The
    /// user's own attachments are judged first, then the disk (off the UI
    /// thread), then the backend.
    pub(super) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.composer.read(cx).value().trim().to_string();
        // Told apart as typed: a paste that starts with `!` or `/` is
        // content, not a command.
        let shell = typed.starts_with('!');
        let text = attachments::expand(&typed, &self.pasted);
        let images = !shell && !self.attachments.is_empty();
        let files = !shell && !self.files.is_empty();
        if text.is_empty() && !images && !files {
            return;
        }
        if shell && (!self.attachments.is_empty() || !self.files.is_empty()) {
            // A shell command carries no attachments; nothing would show them.
            self.warn(t(SHELL_ATTACHMENTS).into(), window, cx);
            return;
        }
        if images && let Some(model) = self.no_image_model() {
            let message = image_warning(model);
            self.warn(message, window, cx);
            return;
        }
        // An attached file is checked against the disk first, off the UI
        // thread: a path on an unresponsive mount must not stall this frame.
        if files {
            self.check_files_then_send(window, cx);
        } else {
            self.send(window, cx);
        }
    }

    /// Ask the disk, on the background executor, whether the attached files
    /// are still there, then send (or warn and leave everything as it is):
    /// the text and the tiles stay untouched until the check passes. What the
    /// press would send is remembered ([`Draft`]), so an answer that lands
    /// after the composer changed — a tile removed, a word typed, an image
    /// attached, another file dropped — sends nothing: that press is over and
    /// another Enter starts the check again. A second Enter while the check
    /// is in flight starts nothing, so one press sends once.
    fn check_files_then_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.checking_files {
            return;
        }
        let asked = self.draft(cx);
        self.checking_files = true;
        let paths: Vec<PathBuf> = asked.files.iter().map(|file| file.path.clone()).collect();
        let check = cx.background_executor().spawn(async move {
            paths
                .into_iter()
                .map(|path| {
                    let there = path.is_file();
                    (path, there)
                })
                .collect::<HashMap<PathBuf, bool>>()
        });
        cx.spawn_in(window, async move |this, cx| {
            let there = check.await;
            let _ = this.update_in(cx, |this, window, cx| {
                // Every answer clears the flag before it decides, so no path
                // leaves it set and the next Enter can ask the disk again.
                this.checking_files = false;
                // The guards `submit` ran before the check, judged here on
                // what the composer holds now: the snapshot is the one the
                // check asked about, so its paths are the ones it answered
                // for, and a path it never saw never counts as gone.
                let now = this.draft(cx);
                let no_image_model = this.no_image_model().map(str::to_string);
                let ready = this.process().is_some();
                let exists = |path: &Path| there.get(path).copied().unwrap_or(true);
                match after_check(&asked, &now, exists, no_image_model.as_deref(), ready) {
                    AfterCheck::Send => this.send(window, cx),
                    AfterCheck::Warn(message) => this.warn(message, window, cx),
                    AfterCheck::Abort => {}
                }
            });
        })
        .detach();
    }

    /// What the composer holds right now, as [`after_check`] reads it.
    fn draft(&self, cx: &App) -> Draft {
        Draft {
            typed: self.composer.read(cx).value().trim().to_string(),
            pasted: self
                .pasted
                .iter()
                .map(|pasted| pasted.text.clone())
                .collect(),
            images: self.attachments.iter().map(|image| image.key).collect(),
            files: self.files.clone(),
        }
    }

    /// The current model's name when it takes no image input: what the image
    /// guard warns with.
    fn no_image_model(&self) -> Option<&str> {
        self.models
            .current
            .as_ref()
            .filter(|model| !model.images)
            .map(|model| model.name.as_str())
    }

    /// Send what the composer holds, once nothing refuses it: `!` shell
    /// commands, built-ins and natively shown commands run here, prompts with
    /// their attachments go to the agent. Reads the composer again, so what it
    /// sends is what is on screen, which [`after_check`] guarantees is what the
    /// press asked for.
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.composer.read(cx).value().trim().to_string();
        let shell = typed.starts_with('!');
        let text = attachments::expand(&typed, &self.pasted);
        if self.process().is_none() {
            return;
        }
        self.composer
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.prompt_history.record(&text);
        let pasted = !std::mem::take(&mut self.pasted).is_empty();
        self.list.scroll_to_end();

        if let Some(command) = text.strip_prefix('!').filter(|_| shell) {
            self.run_shell(command, cx);
            cx.notify();
            return;
        }

        if self.attachments.is_empty()
            && self.files.is_empty()
            && !pasted
            && self.intercept(&text, cx)
        {
            cx.notify();
            return;
        }

        let images = std::mem::take(&mut self.attachments);
        let files = std::mem::take(&mut self.files);
        // What the prompt says: the text, then where the agent reads the files.
        let message = self.prompt_message(&text, &files);
        self.send_prompt_with(message, images, cx);
        cx.notify();
    }

    /// What the prompt's `message` says: the text, then the attached files as
    /// this tab's backend reads their paths (in WSL, `C:\Users\x\a.pdf` is
    /// `/mnt/c/Users/x/a.pdf`).
    pub(super) fn prompt_message(&self, text: &str, files: &[attachments::AttachedFile]) -> String {
        attachments::with_files(text, files, &self.cwd, self.runs_on.as_ref())
    }

    /// Text from outside the composer (`-p`, a later launch's prompt): runs
    /// the way typing it would, `!` shell commands included.
    pub(super) fn run_text(&mut self, text: String, cx: &mut Context<Self>) {
        match text.trim().strip_prefix('!') {
            Some(command) => {
                self.run_shell(command, cx);
                cx.notify();
            }
            None => self.run_prompt(text, cx),
        }
    }

    /// Pi's TUI convention: `!cmd` runs and shares output, `!!cmd` keeps it
    /// out of context. `command` is what follows the first `!`.
    fn run_shell(&mut self, command: &str, cx: &mut Context<Self>) {
        let (command, exclude) = match command.strip_prefix('!') {
            Some(rest) => (rest.trim().to_string(), true),
            None => (command.trim().to_string(), false),
        };
        if command.is_empty() {
            return;
        }
        let index = self.transcript.start_shell(command.clone());
        self.active_shell = Some(index);
        self.request(
            json!({"type": "bash", "command": command, "excludeFromContext": exclude}),
            cx,
            move |this, result, _| {
                this.active_shell = None;
                match result {
                    Ok(data) => this.transcript.finish_shell(
                        index,
                        data["exitCode"].as_i64(),
                        data["output"].as_str().map(str::to_string),
                    ),
                    Err(error) => {
                        this.transcript
                            .finish_shell(index, None, Some(format!("{error:#}")))
                    }
                }
            },
        );
    }

    /// Run a slash command the way typing it would: built-ins and natively
    /// shown commands run here, the rest goes to Pi.
    pub fn run_prompt(&mut self, text: String, cx: &mut Context<Self>) {
        if !self.intercept(&text, cx) {
            self.send_prompt(text, cx);
        }
        cx.notify();
    }

    /// Run `text` here instead of sending it when it is a Pi built-in or a
    /// Gentle Shell command the app shows natively; false: send it.
    fn intercept(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        match builtins::classify(text) {
            builtins::Submit::Builtin(builtin) => {
                self.run_builtin(builtin, cx);
                true
            }
            builtins::Submit::Open(screen) => {
                cx.emit(SessionEvent::OpenScreen(screen));
                true
            }
            builtins::Submit::Prompt => self.intercept_pi_builtin(text, cx),
        }
    }

    /// A Pi built-in the session feed lists (none on a Shell without it):
    /// Pi does not run them over RPC, where the text would reach the model,
    /// so `/new` and `/clone` go through their RPC commands and the rest
    /// say where they work instead of being sent.
    fn intercept_pi_builtin(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let Some((name, _)) = builtins::split_command(text) else {
            return false;
        };
        let Some(command) = self
            .desktop
            .sections()
            .pi_commands()
            .into_iter()
            .find(|command| command.name == name)
        else {
            return false;
        };
        match gentle_core::commands::pi_builtin(&command) {
            PiBuiltin::NewSession => self.replace_session(json!({"type": "new_session"}), cx),
            PiBuiltin::Clone => self.replace_session(json!({"type": "clone"}), cx),
            PiBuiltin::Elsewhere { terminal } => {
                let name = name.to_string();
                let key = if terminal {
                    "/{name} only works in Gentle Shell's terminal."
                } else {
                    "/{name} isn't available in the app yet."
                };
                self.localized_note(move || t(key).replace("{name}", &name), cx);
            }
        }
        true
    }

    /// `/web <url>`: show a page in the in-app browser instead of sending the
    /// command to Gentle Shell.
    fn open_web(&mut self, url: String, cx: &mut Context<Self>) {
        let url = url.trim();
        if url.is_empty() {
            self.localized_note(|| t("Usage: /web <url>").to_string(), cx);
        } else if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.localized_note(
                || t("Only http:// and https:// URLs can open here.").to_string(),
                cx,
            );
        } else {
            cx.emit(SessionEvent::OpenWeb(url.to_string()));
        }
    }

    fn run_builtin(&mut self, builtin: builtins::Builtin, cx: &mut Context<Self>) {
        match builtin {
            builtins::Builtin::Compact(instructions) => self.compact(instructions, cx),
            builtins::Builtin::Name(name) if name.trim().is_empty() => {
                let name = self.session_name.clone();
                self.localized_note(
                    move || match &name {
                        Some(name) => {
                            t("This session is named “{name}”. Rename it with /name <title>.")
                                .replace("{name}", name)
                        }
                        None => {
                            t("This session has no name yet. Name it with /name <title>.").into()
                        }
                    },
                    cx,
                );
            }
            builtins::Builtin::Name(name) => self.rename(name, cx),
            builtins::Builtin::Copy => self.copy_last_answer(cx),
            builtins::Builtin::Export(path) => self.export_html(path, cx),
            builtins::Builtin::Reload => self.reload(cx),
            builtins::Builtin::Web(url) => self.open_web(url, cx),
            builtins::Builtin::UiTemplate(name) => self.ui_template(&name, cx),
        }
    }

    /// `/name ` in the composer, cursor at its end.
    pub(super) fn insert_command(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = format!("/{name} ");
        let column = text.chars().map(char::len_utf16).sum::<usize>() as u32;
        self.composer.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.set_cursor_position(Position::new(0, column), window, cx);
            state.focus(window, cx);
        });
        // The whole text was replaced: no paste's token is left in it.
        self.pasted.clear();
    }

    /// A `/` menu row with a native screen, picked: the screen opens on this
    /// one press, and the command leaves the composer as a sent one does. It
    /// never becomes a prompt — not even with a file or an image waiting
    /// beside the text: the person asked for the screen, not the model.
    pub(super) fn open_screen_command(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = format!("/{name}");
        self.composer
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.prompt_history.record(&text);
        self.pasted.clear();
        self.intercept(&text, cx);
        cx.notify();
    }

    /// The reference a picked skill leaves in the composer: `/skill:name` as
    /// one inline token, marked in the theme's accent (see the composer's
    /// token renderer), a space after it and the caret there, ready for the
    /// instructions that follow. The text is the command itself, so sending
    /// the prompt invokes the skill; a skill opens nothing, so nothing is
    /// opened or sent here. Only the `/query` at the caret is replaced; a query
    /// gone from the caret (or no longer naming the row) picks nothing.
    pub(super) fn insert_skill_reference(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.composer.read(cx);
        let (text, cursor) = (state.value().to_string(), state.cursor());
        let Some(query) = slash_query_at(&text, cursor) else {
            return;
        };
        // The menu matched the row from that query with `contains`: stale now?
        if !name.contains(&text[query.start + 1..query.end]) {
            return;
        }
        let mut reference = format!("/{name}");
        // The space after it is the one already there, when there is one.
        if !text[query.end..].starts_with(char::is_whitespace) {
            reference.push(' ');
        }
        let caret = query.start + reference.len();
        let text = format!("{}{reference}{}", &text[..query.start], &text[query.end..]);
        self.set_composer_text(text, caret, window, cx);
        self.composer
            .update(cx, |state, cx| state.focus(window, cx));
    }

    /// Attach what was dropped or picked and warn about the rest: images wait
    /// as thumbnails, files as cards, both in the strip above the composer.
    /// Nothing is typed into the text, and a file or image already waiting is
    /// not attached twice (one card either way).
    pub(super) fn attach(
        &mut self,
        loaded: Vec<Result<attachments::Loaded, attachments::Rejection>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut rejected = Vec::new();
        for result in loaded {
            match result {
                Ok(attachments::Loaded::Image(image)) => {
                    // The same bytes (a screenshot pasted twice, a file
                    // dropped twice) are the same image.
                    if !self.attachments.iter().any(|a| a.key == image.key) {
                        self.attachments.push(image);
                    }
                }
                Ok(attachments::Loaded::File(file)) => {
                    // One file is one tile, however its path is spelled (a
                    // symlink, a `./` in it): the canonical path decides.
                    if !self.files.iter().any(|f| f.canonical == file.canonical) {
                        self.files.push(file);
                    }
                }
                Err(rejection) => rejected.push(rejection),
            }
        }
        self.composer
            .update(cx, |state, cx| state.focus(window, cx));
        if let Some(message) =
            attachments::rejection_warning_in(crate::shared::i18n::language(), &rejected)
        {
            self.warn(message, window, cx);
        }
        cx.notify();
    }

    /// Read dropped or picked files off the UI thread, then attach them.
    pub(super) fn attach_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        // The prompt's naming of the files decides what a path may carry (a
        // folder above them that it never names can't stop the attach), and
        // the prompt names them as the tab's backend reads them.
        let cwd = self.cwd.clone();
        let backend = self.runs_on.clone();
        self.attach_off_thread(
            move || {
                paths
                    .iter()
                    .map(|path| attachments::load_path(path, &cwd, backend.as_ref()))
                    .collect()
            },
            window,
            cx,
        );
    }

    /// Run `load` on the background executor, then attach what it read.
    fn attach_off_thread(
        &mut self,
        load: impl FnOnce() -> Vec<Result<attachments::Loaded, attachments::Rejection>> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let loading = cx.background_executor().spawn(async move { load() });
        cx.spawn_in(window, async move |this, cx| {
            let loaded = loading.await;
            let _ = this.update_in(cx, |this, window, cx| this.attach(loaded, window, cx));
        })
        .detach();
    }

    /// The composer's paste hook: clipboard images and copied files become
    /// attachments (a copied folder is refused, as when dropped), and so does
    /// long text ([`Self::paste_text`]); anything else pastes as text.
    pub(super) fn paste_clipboard(
        &mut self,
        item: &ClipboardItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut images = Vec::new();
        let mut paths = Vec::new();
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) => images.push(image.bytes.clone()),
                ClipboardEntry::ExternalPaths(external) => {
                    paths.extend(external.paths().iter().cloned())
                }
                _ => {}
            }
        }
        if images.is_empty() && paths.is_empty() {
            return item
                .text()
                .is_some_and(|text| self.paste_text(text, window, cx));
        }
        if !images.is_empty() {
            // Off the UI thread: a BMP or TIFF screenshot is re-encoded first.
            self.attach_off_thread(
                move || {
                    images
                        .into_iter()
                        .map(|bytes| {
                            attachments::Attachment::from_bytes(t("The pasted image"), bytes)
                                .map(attachments::Loaded::Image)
                        })
                        .collect()
                },
                window,
                cx,
            );
        }
        self.attach_paths(paths, window, cx);
        true
    }

    /// Keep a long paste ([`attachments::is_long_paste`]) as an attachment,
    /// its `[Pasted text #N]` token at the cursor shown as a chip (a file
    /// icon and `#N`, see the composer's token renderer);
    /// false: it's short, paste it as text.
    fn paste_text(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !attachments::is_long_paste(&text) {
            return false;
        }
        self.pasted.push(attachments::PastedText::new(text));
        let n = self.pasted.len();
        self.composer.update(cx, |state, cx| {
            let value = state.value();
            let before = value[..state.cursor().min(value.len())].chars().next_back();
            if attachments::needs_lead(before) {
                state.replace(" ", window, cx);
            }
            if state
                .replace_with_token(pasted_chip(n), window, cx)
                .is_err()
            {
                // Mid-composition the chip can't go in; the token still works.
                state.replace(attachments::token(n), window, cx);
            }
            state.insert(" ", window, cx);
            state.focus(window, cx);
        });
        cx.notify();
        true
    }

    /// The 📎 button: pick files or images.
    pub(super) fn pick_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t("Attach").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| this.attach_paths(paths, window, cx));
        })
        .detach();
    }

    /// Drop image `index` (0-based) from the strip; the text stays as typed.
    pub(super) fn remove_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.attachments.len() {
            return;
        }
        self.attachments.remove(index);
        cx.notify();
    }

    /// Drop file `index` (0-based) from the strip; the text stays as typed.
    pub(super) fn remove_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.files.len() {
            return;
        }
        self.files.remove(index);
        cx.notify();
    }

    /// Drop pasted text `index` (0-based) and its token; later ones shift down.
    pub(super) fn remove_pasted(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.pasted.len() {
            return;
        }
        self.pasted.remove(index);
        let text = attachments::remove_token(&self.composer.read(cx).value(), index + 1);
        let end = text.len();
        self.set_composer_text(text, end, window, cx);
        cx.notify();
    }

    /// After an edit: drop the pastes whose token left the text (deleted,
    /// cut, typed over) and renumber the rest, as removing their cards would.
    pub(super) fn sync_pasted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pasted.is_empty() {
            return;
        }
        let (text, cursor) = {
            let state = self.composer.read(cx);
            (state.value(), state.cursor().min(state.value().len()))
        };
        let kept = attachments::present(&text, self.pasted.len());
        if kept.len() == self.pasted.len() {
            return;
        }
        let mut number = 0;
        self.pasted.retain(|_| {
            number += 1;
            kept.binary_search(&number).is_ok()
        });
        let cursor = attachments::renumber(&text[..cursor], &kept).len();
        let text = attachments::renumber(&text, &kept);
        self.set_composer_text(text, cursor, window, cx);
        cx.notify();
    }

    /// Put `text` in the composer with the pastes' tokens as chips, the known
    /// skills' references marked and the cursor at byte `cursor`. Setting the
    /// value clears the undo history, so undo can't bring back the token of a
    /// paste that is gone.
    fn set_composer_text(
        &mut self,
        text: String,
        cursor: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = position_at(&text, cursor);
        let chips = attachments::find_tokens(&text);
        // A skill's reference keeps its marking through a rebuild (a paste
        // card removed, a prompt recalled): the command is still the one the
        // prompt invokes.
        let references = self.skill_references(&text);
        let mut content = InputContent::new(text);
        for (range, n) in chips {
            if (1..=self.pasted.len()).contains(&n) {
                content = content
                    .clone()
                    .with_token(range, pasted_chip(n))
                    .unwrap_or(content);
            }
        }
        for (range, token) in references {
            content = content.clone().with_token(range, token).unwrap_or(content);
        }
        self.composer.update(cx, |state, cx| {
            state.set_value(content, window, cx);
            state.set_cursor_position(position, window, cx);
        });
    }

    /// The known skills' references in `text`, each with the token marking it:
    /// a `/skill:name` that this tab's backend lists as a skill, alone between
    /// whitespace (or the ends of the text). A command that only looks like
    /// one — a name no skill has, a longer name, one glued to the word before
    /// it — is left as plain text, as is one for an unknown skill.
    fn skill_references(&self, text: &str) -> Vec<(Range<usize>, InlineToken)> {
        let skills: Vec<String> = self
            .commands()
            .into_iter()
            .map(|command| format!("/{}", command.name))
            .filter(|command| command.starts_with("/skill:"))
            .collect();
        if skills.is_empty() {
            return Vec::new();
        }
        let mut references = Vec::new();
        for (start, _) in text.match_indices("/skill:") {
            if text[..start]
                .chars()
                .next_back()
                .is_some_and(|before| !before.is_whitespace())
            {
                continue;
            }
            let rest = &text[start..];
            let end = start + rest.find(char::is_whitespace).unwrap_or(rest.len());
            let command = &text[start..end];
            if skills.iter().any(|skill| skill == command) {
                references.push((start..end, skill_reference(command)));
            }
        }
        references
    }

    /// ↑ on the composer's first line recalls the prompt sent before, newest
    /// first; ↓ on its last line walks back toward the newest, then to the
    /// draft that was being written. Nothing is sent. Lines in between move
    /// the cursor as usual, the `/` menu keeps its ↑/↓ until history is
    /// entered, and a composer holding long pastes is left alone (a recalled
    /// prompt would drop their tokens). Images and files wait beside the
    /// text, not in it, so they stay attached.
    pub(in crate::features::session) fn history_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt || m.shift {
            return false;
        }
        let up = match keystroke.key.as_str() {
            "up" => true,
            "down" => false,
            _ => return false,
        };
        if !self.composer.focus_handle(cx).is_focused(window) || !self.pasted.is_empty() {
            return false;
        }
        let (text, line) = {
            let state = self.composer.read(cx);
            (state.value().to_string(), state.cursor_position().line)
        };
        let browsing = self.prompt_history.browsing();
        if !browsing && self.slash_menu_open(cx) {
            return false;
        }
        let recalled = if up {
            if line != 0 {
                return false;
            }
            self.prompt_history.older(&text)
        } else {
            if !browsing || line as usize != text.matches('\n').count() {
                return false;
            }
            self.prompt_history.newer()
        };
        let Some(recalled) = recalled.map(str::to_string) else {
            // At the oldest prompt ↑ stays put; with no history it's the
            // composer's own.
            return browsing;
        };
        let end = recalled.len();
        self.set_composer_text(recalled, end, window, cx);
        cx.notify();
        true
    }
}

/// What the composer would send, as the disk check's decision reads it: the
/// text as typed (a leading `!` makes it a shell command), the pastes it
/// carries, the images' keys and the files as attached (their canonical path
/// tells two spellings of one file apart). Two equal drafts send one prompt,
/// so an answer that finds a different one in the composer answers a press
/// that is over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Draft {
    /// The composer's text, trimmed: what `!` and `/` are read from.
    typed: String,
    /// Each waiting paste's text, in order: a card removed while the check
    /// ran (or one that lost its token) is a different draft.
    pasted: Vec<String>,
    /// The waiting images' keys, in strip order.
    images: Vec<u64>,
    /// The waiting files, in strip order.
    files: Vec<attachments::AttachedFile>,
}

/// What the disk check decides, from what the press would send, what the
/// composer holds as the answer lands and what the check found.
#[derive(Debug, PartialEq, Eq)]
enum AfterCheck {
    /// Send the press: the composer still holds it, no guard refuses it and
    /// the backend is running.
    Send,
    /// The composer still holds the press, but a guard refuses it: say why,
    /// and change nothing else.
    Warn(String),
    /// Do nothing at all: the press is over (the composer holds something
    /// else now), it would send nothing, or there is no backend to send to.
    Abort,
}

/// What to do once the disk answered, as a rule of its own: `asked` is the
/// draft the press would have sent, `now` what the composer holds as the
/// answer lands, `there` whether an attached path is still a file, `model`
/// the current model's name when it takes no image input, and `ready` whether
/// the tab has a backend to send to. Every guard [`SessionView::submit`] ran
/// before the check runs again here, on the composer's state now, so `send`
/// is never reached with something a first Enter already refuses.
fn after_check(
    asked: &Draft,
    now: &Draft,
    there: impl Fn(&Path) -> bool,
    model: Option<&str>,
    ready: bool,
) -> AfterCheck {
    // The person changed the composer while the disk was asked: that press is
    // over. Nothing is sent, cleared or remembered, and no warning explains a
    // change they made themselves; another Enter starts the check again.
    if asked != now {
        return AfterCheck::Abort;
    }
    if now.typed.is_empty() && now.images.is_empty() && now.files.is_empty() {
        return AfterCheck::Abort;
    }
    // The guards `submit` ran before the check, on the composer as it is now.
    let shell = now.typed.starts_with('!');
    if shell && (!now.images.is_empty() || !now.files.is_empty()) {
        return AfterCheck::Warn(t(SHELL_ATTACHMENTS).to_string());
    }
    if !shell
        && !now.images.is_empty()
        && let Some(model) = model
    {
        return AfterCheck::Warn(image_warning(model));
    }
    // Then the disk's own answer, and a backend to send to.
    if let Some(message) =
        attachments::missing_files_in(crate::shared::i18n::language(), &now.files, there)
    {
        return AfterCheck::Warn(message);
    }
    if !ready {
        return AfterCheck::Abort;
    }
    AfterCheck::Send
}

/// The chip pasted text `n` shows as in the composer over its
/// `[Pasted text #N]` token: `#N`, after the renderer's file icon. The
/// icon is not a `📄` in the label: gpui-kit's tokens draw stray glyphs
/// after an emoji label.
fn pasted_chip(n: usize) -> InlineToken {
    InlineToken::new("pasted-text", attachments::token(n)).with_label(format!("#{n}"))
}

/// The id of the inline token marking a skill the prompt invokes, so the
/// composer draws it in the theme's accent instead of a paste's chip.
pub(super) const SKILL_TOKEN_ID: &str = "skill-reference";

/// The token a skill's reference is in the composer: its own `/skill:name`,
/// kept as one editing unit and labeled as itself.
fn skill_reference(command: &str) -> InlineToken {
    InlineToken::new(SKILL_TOKEN_ID, command)
}

/// The `/query` the caret ends, from its `/`; None when the caret left it.
fn slash_query_at(text: &str, cursor: usize) -> Option<Range<usize>> {
    let caret = cursor.min(text.len());
    let before = &text[..caret];
    let start = before.rfind('/')?;
    let query = &before[start..];
    let word = before[..start].chars().next_back();
    (!query.contains(char::is_whitespace) && word.is_none_or(char::is_whitespace))
        .then_some(start..caret)
}

/// The editor position (line, UTF-16 column) of byte `offset` in `text`.
fn position_at(text: &str, offset: usize) -> Position {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() as u32;
    let column = before
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .map(char::len_utf16)
        .sum::<usize>() as u32;
    Position::new(line, column)
}

#[cfg(test)]
mod tests {
    // Name what is needed: a glob would bring gpui's `test` attribute, which
    // would shadow `#[test]` (see `view_tests`).
    use std::path::{Path, PathBuf};

    use super::{AfterCheck, Draft, SHELL_ATTACHMENTS, after_check, attachments, image_warning};

    /// A file as the composer holds it; `path` is what the prompt names.
    fn file(path: &str) -> attachments::AttachedFile {
        attachments::AttachedFile {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            path: PathBuf::from(path),
            canonical: PathBuf::from(path),
            bytes: 12,
        }
    }

    /// A draft holding only `typed`.
    fn typed(text: &str) -> Draft {
        Draft {
            typed: text.to_string(),
            ..Draft::default()
        }
    }

    /// The decision for `asked` and `now`, with every file still on disk, a
    /// model that takes images and a running backend.
    fn decide(asked: &Draft, now: &Draft) -> AfterCheck {
        after_check(asked, now, |_: &Path| true, None, true)
    }

    /// The press and the composer agree, the file is there and nothing
    /// refuses it: it is sent.
    #[test]
    fn an_unchanged_press_with_its_files_there_is_sent() {
        let asked = Draft {
            typed: "look".into(),
            pasted: vec!["a long\npaste".into()],
            images: vec![7],
            files: vec![file("/work/a.pdf")],
        };
        assert_eq!(decide(&asked, &asked.clone()), AfterCheck::Send);
    }

    /// A file gone from disk while the check ran is named, one per line, and
    /// nothing is sent.
    #[test]
    fn a_gone_file_is_named_and_nothing_is_sent() {
        let asked = Draft {
            files: vec![file("/work/a.pdf"), file("/work/b.txt")],
            ..Draft::default()
        };
        let there = |path: &Path| !path.ends_with("b.txt");
        assert_eq!(
            after_check(&asked, &asked.clone(), there, None, true),
            AfterCheck::Warn(
                "b.txt is no longer there; remove it from the attachments to send.".to_string()
            )
        );
    }

    /// The press is the one that was asked about: a composer that changed
    /// while the disk was checked sends nothing, says nothing and remembers
    /// nothing. Another Enter starts over.
    #[test]
    fn a_composer_changed_while_the_check_ran_sends_nothing() {
        let asked = Draft {
            typed: "look".into(),
            pasted: vec!["a long\npaste".into()],
            images: vec![7],
            files: vec![file("/work/a.pdf")],
        };
        let changed: [(&str, Draft); 5] = [
            (
                "the tile was removed",
                Draft {
                    files: Vec::new(),
                    ..asked.clone()
                },
            ),
            (
                "the text was typed over",
                Draft {
                    typed: "look at this".into(),
                    ..asked.clone()
                },
            ),
            (
                "the pasted card was removed",
                Draft {
                    pasted: Vec::new(),
                    ..asked.clone()
                },
            ),
            (
                "the image was removed",
                Draft {
                    images: Vec::new(),
                    ..asked.clone()
                },
            ),
            (
                "another file was attached",
                Draft {
                    files: vec![file("/work/a.pdf"), file("/work/b.txt")],
                    ..asked.clone()
                },
            ),
        ];
        for (what, now) in changed {
            assert_eq!(decide(&asked, &now), AfterCheck::Abort, "{what}");
        }
        // A `!` command typed with the file still attached changed the text,
        // so the press is over before the shell guard is reached (see
        // `a_shell_command_never_carries_attachments` for that guard).
        let shell = Draft {
            typed: "!ls".into(),
            ..asked.clone()
        };
        assert_eq!(decide(&asked, &shell), AfterCheck::Abort);
    }

    /// A `!` command never carries attachments: were the composer to hold one
    /// when the answer lands, it is refused instead of run.
    #[test]
    fn a_shell_command_never_carries_attachments() {
        let shell = Draft {
            typed: "!ls".into(),
            files: vec![file("/work/a.pdf")],
            ..Draft::default()
        };
        assert_eq!(
            decide(&shell, &shell.clone()),
            AfterCheck::Warn(SHELL_ATTACHMENTS.to_string())
        );
        // The same with an image, and a shell command with none is sent.
        let images = Draft {
            images: vec![7],
            ..shell.clone()
        };
        assert_eq!(
            decide(&images, &images.clone()),
            AfterCheck::Warn(SHELL_ATTACHMENTS.to_string())
        );
        let alone = typed("!ls");
        assert_eq!(decide(&alone, &alone.clone()), AfterCheck::Send);
    }

    /// A model switched to one without image input while the check ran is
    /// named when the answer lands, and nothing is sent.
    #[test]
    fn a_model_without_image_input_refuses_the_images() {
        let asked = Draft {
            typed: "look".into(),
            images: vec![7],
            ..Draft::default()
        };
        assert_eq!(
            after_check(&asked, &asked.clone(), |_: &Path| true, Some("gpt-4"), true),
            AfterCheck::Warn(image_warning("gpt-4"))
        );
        assert_eq!(
            image_warning("gpt-4"),
            "gpt-4 does not accept images. Switch to a model with image input, or remove the attachments."
        );
        // A model that takes images sends them.
        assert_eq!(decide(&asked, &asked.clone()), AfterCheck::Send);
    }

    /// Nothing to send, or nowhere to send it: the answer changes nothing. A
    /// draft is trimmed as the composer's text is ([`SessionView::draft`]),
    /// so blank text is an empty draft here.
    #[test]
    fn an_empty_press_or_a_missing_backend_sends_nothing() {
        let empty = Draft::default();
        assert_eq!(decide(&empty, &empty.clone()), AfterCheck::Abort);
        let asked = Draft {
            typed: "look".into(),
            files: vec![file("/work/a.pdf")],
            ..Draft::default()
        };
        assert_eq!(
            after_check(&asked, &asked.clone(), |_: &Path| true, None, false),
            AfterCheck::Abort
        );
    }
}

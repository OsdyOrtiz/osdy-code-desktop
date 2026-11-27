//! Configuration › Appearance › Fonts: a searchable picker of the installed
//! font families for the interface, the code and the terminal, with
//! ↑/↓/Enter/Esc. Monospace families come first for code and terminal.
//! Hovering a family or moving to it previews it in the Reading Desk only;
//! a click or Enter applies it at once, with a toast that undoes it.

use std::collections::HashSet;
use std::rc::Rc;

use gentle_core::fonts::{self, MAX_SIZE, MIN_SIZE};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::SettingsView;
use crate::platform::font_metadata;
use crate::shared::i18n::t;
use crate::shared::themes::{self, FontSlot, Fonts};
use crate::shared::ui;

/// `None` is the Default row, shown only with a blank search.
fn picker_entries(
    names: &[String],
    query: &str,
    monospace: Option<&HashSet<String>>,
) -> Vec<Option<String>> {
    let rows = fonts::picker_rows(names, query, monospace.is_some(), |name| {
        monospace.is_some_and(|families| families.contains(&name.to_lowercase()))
    });
    query
        .trim()
        .is_empty()
        .then_some(None)
        .into_iter()
        .chain(rows.into_iter().map(Some))
        .collect()
}

fn entry_cursor(entries: &[Option<String>], selected: Option<&Option<String>>) -> usize {
    entries
        .iter()
        .position(|entry| Some(entry) == selected)
        .unwrap_or(0)
}

/// What a size field's text means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SizeInput {
    Valid(f32),
    /// Still being typed: empty, `12.`, or a prefix of a valid size (`1`–`3`).
    Incomplete,
    /// Not a number, or out of range.
    Invalid,
}

pub(super) fn parse_size(text: &str) -> SizeInput {
    let text = text.trim();
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if text.is_empty() {
        return SizeInput::Incomplete;
    }
    if let Some(whole) = text.strip_suffix('.') {
        return if !whole.is_empty() && digits(whole) {
            SizeInput::Incomplete
        } else {
            SizeInput::Invalid
        };
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || !digits(whole) || !digits(fraction) {
        return SizeInput::Invalid;
    }
    if fraction.is_empty() && matches!(whole, "1" | "2" | "3") {
        return SizeInput::Incomplete;
    }
    match text.parse::<f32>() {
        Ok(size) if (MIN_SIZE..=MAX_SIZE).contains(&size) => SizeInput::Valid(size),
        _ => SizeInput::Invalid,
    }
}

fn format_size(size: f32) -> String {
    if size.fract() == 0. {
        format!("{size:.0}")
    } else {
        format!("{size}")
    }
}

fn slot_index(slot: FontSlot) -> usize {
    match slot {
        FontSlot::Interface => 0,
        FontSlot::Code => 1,
        FontSlot::Terminal => 2,
    }
}

/// One applied font change, holding what the slot had before so the toast
/// can put it back.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Change {
    Family(FontSlot, Option<SharedString>),
    Size(FontSlot, f32),
}

impl Change {
    /// `current` with this change's slot put back; every other slot keeps
    /// what it has now, so undoing an older change never reverts newer ones.
    pub(super) fn revert(&self, current: &Fonts) -> Fonts {
        let mut fonts = current.clone();
        match self {
            Change::Family(slot, family) => fonts.set(*slot, family.clone()),
            Change::Size(slot, size) => fonts.set_size(*slot, *size),
        }
        fonts
    }

    /// The toast after the change, naming what `applied` now uses.
    pub(super) fn message(&self, applied: &Fonts) -> String {
        match *self {
            Change::Family(slot, _) => match (applied.get(slot), slot) {
                (Some(family), _) => t("{slot} changed to {family}. Click to undo.")
                    .replace("{slot}", slot_label(slot))
                    .replace("{family}", family),
                (None, FontSlot::Terminal) => {
                    t("Terminal font now follows the code font. Click to undo.").to_string()
                }
                (None, _) => {
                    t("{slot} reset to default. Click to undo.").replace("{slot}", slot_label(slot))
                }
            },
            Change::Size(slot, _) => t("{slot} size changed to {size} px. Click to undo.")
                .replace("{slot}", slot_label(slot))
                .replace("{size}", &format_size(applied.size(slot))),
        }
    }
}

/// Marks the font toast, so a newer change replaces it.
struct FontToast;

/// What the Reading Desk draws: the applied fonts, with the family under the
/// open picker's cursor (`peek`) and any valid size being typed laid over
/// them. Nothing here reaches the app.
pub(super) fn preview_fonts(
    applied: &Fonts,
    peek: Option<(FontSlot, Option<&str>)>,
    texts: &[String; 3],
) -> Fonts {
    let mut fonts = applied.clone();
    if let Some((slot, family)) = peek {
        fonts.set(
            slot,
            family.map(|name| SharedString::from(name.to_string())),
        );
    }
    for slot in FontSlot::ALL {
        if let SizeInput::Valid(size) = parse_size(&texts[slot_index(slot)]) {
            fonts.set_size(slot, size);
        }
    }
    fonts
}

/// The family `fonts` draws `slot` with: its own, else its default; the
/// terminal's default is the code font.
pub(super) fn effective_family(
    fonts: &Fonts,
    slot: FontSlot,
    default_interface: &SharedString,
) -> SharedString {
    match (fonts.get(slot), slot) {
        (Some(family), _) => family.clone(),
        (None, FontSlot::Interface) => default_interface.clone(),
        (None, FontSlot::Code) => ui::default_mono(),
        (None, FontSlot::Terminal) => fonts
            .terminal_family()
            .cloned()
            .unwrap_or_else(ui::default_mono),
    }
}

/// The picker's state: which slot's list is open, the search, the cursor.
pub(super) struct FontsUi {
    /// The text of each size field, in `FontSlot::ALL` order; a valid one
    /// previews until Enter or leaving the field applies it.
    texts: [String; 3],
    /// One number field per slot, in `FontSlot::ALL` order.
    sizes: [Entity<InputState>; 3],
    /// Whether the Reading Desk is expanded.
    pub(super) desk_open: bool,
    pub(super) open: Option<FontSlot>,
    search: Entity<InputState>,
    /// Index into the entries: the Default entry (when shown), then families.
    /// Hover and ↑/↓ both move it; its family is what the desk previews.
    pub(super) cursor: usize,
    scroll: UniformListScrollHandle,
    /// The installed families, read when the picker opens.
    names: Vec<String>,
    pub(super) entries: Rc<[Option<String>]>,
    query: String,
    waiting_for_metadata: bool,
    _subscriptions: Vec<Subscription>,
}

impl FontsUi {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SettingsView>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t("Search fonts…")));
        let weak = cx.entity().downgrade();
        let applied = cx.global::<themes::Themes>().fonts.clone();
        let texts = FontSlot::ALL.map(|slot| format_size(applied.size(slot)));
        let sizes = texts
            .clone()
            .map(|text| cx.new(|cx| InputState::new(window, cx).default_value(text)));
        let mut subscriptions = vec![
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event
                    && this.fonts.open.is_some()
                {
                    let query = this.fonts.search.read(cx).value().to_string();
                    // Opening the picker also clears the input programmatically.
                    if query != this.fonts.query {
                        this.fonts.query = query;
                        this.fonts.rebuild_entries();
                        this.fonts.select_cursor(0);
                        cx.notify();
                    }
                }
            }),
            // ↑/↓/Enter/Esc in the open picker beat its focused search field
            // and the workspace's Esc, which would close the whole screen.
            cx.intercept_keystrokes(move |event, window, cx| {
                let consumed = weak
                    .update(cx, |this, cx| this.fonts_key(&event.keystroke, window, cx))
                    .unwrap_or(false);
                if consumed {
                    cx.stop_propagation();
                }
            }),
        ];
        for (slot, input) in FontSlot::ALL.into_iter().zip(&sizes) {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        this.fonts.texts[slot_index(slot)] = input.read(cx).value().to_string();
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        this.commit_size(slot, window, cx);
                    }
                    InputEvent::Focus => {}
                },
            ));
        }
        // An undo from the toast (or any other font change) shows in the fields.
        subscriptions.push(
            cx.observe_global_in::<themes::Themes>(window, |this, window, cx| {
                this.sync_size_inputs(window, cx);
            }),
        );
        Self {
            texts,
            sizes,
            desk_open: true,
            open: None,
            search,
            cursor: 0,
            scroll: UniformListScrollHandle::new(),
            names: Vec::new(),
            entries: Rc::from([]),
            query: String::new(),
            waiting_for_metadata: false,
            _subscriptions: subscriptions,
        }
    }

    fn rebuild_entries(&mut self) {
        let monospace = match self.open {
            Some(FontSlot::Code | FontSlot::Terminal) => font_metadata::cached_monospace(),
            _ => None,
        };
        self.entries = picker_entries(&self.names, &self.query, monospace).into();
    }

    fn select_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
        self.scroll.scroll_to_item(cursor, ScrollStrategy::Nearest);
    }
}

/// A virtual list as tall as its rows, up to `max`, then scrolling. Inside
/// a popover, which has no height of its own, a `uniform_list` alone either
/// collapses to nothing or, sized from its rows, stops at its own cap while
/// still stretching the popover to every row; the wrapper holds the cap.
fn fit_rows(list: UniformList, max: Rems) -> Div {
    v_flex().max_h(max).child(
        list.with_sizing_behavior(ListSizingBehavior::Infer)
            .flex_1()
            .min_h_0(),
    )
}

fn slot_label(slot: FontSlot) -> &'static str {
    match slot {
        FontSlot::Interface => t("Interface font"),
        FontSlot::Code => t("Code font"),
        FontSlot::Terminal => t("Terminal font"),
    }
}

/// What the Default entry says for `slot`.
fn default_label(slot: FontSlot) -> &'static str {
    match slot {
        FontSlot::Terminal => t("Same as code font"),
        _ => t("Default"),
    }
}

impl SettingsView {
    fn request_font_metadata(&mut self, cx: &mut Context<Self>) {
        if self.fonts.waiting_for_metadata || font_metadata::cached_monospace().is_some() {
            return;
        }
        self.fonts.waiting_for_metadata = true;
        let task = cx.background_executor().spawn(async {
            font_metadata::load_monospace();
        });
        cx.spawn(async move |this, cx| {
            task.await;
            let _ = this.update(cx, |this, cx| {
                let fonts = &mut this.fonts;
                fonts.waiting_for_metadata = false;
                if matches!(fonts.open, Some(FontSlot::Code | FontSlot::Terminal)) {
                    // The user may have searched, navigated, or switched slots
                    // while the metadata was loading. Keep the current row by value.
                    let selected = fonts.entries.get(fonts.cursor).cloned();
                    fonts.rebuild_entries();
                    fonts.select_cursor(entry_cursor(&fonts.entries, selected.as_ref()));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Open the picker for `slot` (closing any other), or close it.
    fn set_fonts_menu(
        &mut self,
        slot: Option<FontSlot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.fonts.open == slot {
            return;
        }
        self.fonts.open = slot;
        if let Some(slot) = slot {
            self.fonts.names = cx.text_system().all_font_names();
            self.fonts.query.clear();
            self.fonts.search.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.focus(window, cx);
            });
            // Start on the slot's own family, so Enter keeps it.
            let current = cx
                .global::<themes::Themes>()
                .fonts
                .get(slot)
                .map(|f| f.to_string());
            if slot != FontSlot::Interface {
                self.request_font_metadata(cx);
            }
            self.fonts.rebuild_entries();
            self.fonts
                .select_cursor(entry_cursor(&self.fonts.entries, Some(&current)));
        }
        cx.notify();
    }

    /// ↑/↓/Enter/Esc while a fonts picker is open.
    fn fonts_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if keystroke.key == "escape" && self.cancel_size_edit(window, cx) {
            return true;
        }
        let Some(slot) = self.fonts.open else {
            return false;
        };
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt {
            return false;
        }
        let key = keystroke.key.as_str();
        if key == "escape" {
            self.set_fonts_menu(None, window, cx);
            return true;
        }
        if !matches!(key, "up" | "down" | "enter") {
            return false;
        }
        let entries = &self.fonts.entries;
        let n = entries.len();
        let cursor = self.fonts.cursor.min(n.saturating_sub(1));
        match key {
            "up" | "down" if n > 0 => {
                let cursor = if key == "up" {
                    (cursor + n - 1) % n
                } else {
                    (cursor + 1) % n
                };
                self.fonts.select_cursor(cursor);
            }
            "up" | "down" => {}
            "enter" => {
                if let Some(entry) = entries.get(cursor).cloned() {
                    self.choose_font(slot, entry.as_deref(), window, cx);
                }
            }
            _ => return false,
        }
        cx.notify();
        true
    }

    /// Apply `family` to `slot` at once, with a toast that undoes it.
    fn choose_font(
        &mut self,
        slot: FontSlot,
        family: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_fonts_menu(None, window, cx);
        let applied = cx.global::<themes::Themes>().fonts.clone();
        let mut next = applied.clone();
        next.set(
            slot,
            family.map(|name| SharedString::from(name.to_string())),
        );
        if next != applied {
            let before = applied.get(slot).cloned();
            self.apply_font_change(next, Change::Family(slot, before), window, cx);
        }
        cx.notify();
    }

    /// Enter or leaving a size field: apply a valid new size, else put the
    /// applied one back in the field.
    fn commit_size(&mut self, slot: FontSlot, window: &mut Window, cx: &mut Context<Self>) {
        let applied = cx.global::<themes::Themes>().fonts.clone();
        let before = applied.size(slot);
        match parse_size(&self.fonts.texts[slot_index(slot)]) {
            SizeInput::Valid(size) if size != before => {
                let mut next = applied;
                next.set_size(slot, size);
                self.apply_font_change(next, Change::Size(slot, before), window, cx);
            }
            _ => {
                self.fonts.texts[slot_index(slot)] = format_size(before);
                self.set_size_input(slot, window, cx);
            }
        }
        cx.notify();
    }

    fn apply_font_change(
        &mut self,
        next: Fonts,
        change: Change,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        themes::apply_fonts(next, cx);
        // What the app really uses now: an uninstalled family is dropped.
        let applied = cx.global::<themes::Themes>().fonts.clone();
        let note = Notification::info(change.message(&applied))
            .id::<FontToast>()
            .on_click(move |_, window, cx| {
                // The click runs inside the notice's own update; removing it
                // now would read it, so wait for that update to end.
                let change = change.clone();
                window.defer(cx, move |window, cx| {
                    let current = cx.global::<themes::Themes>().fonts.clone();
                    themes::apply_fonts(change.revert(&current), cx);
                    window.remove_notification::<FontToast>(cx);
                });
            });
        window.push_notification(note, cx);
    }

    /// Esc in a size field: put the applied size back and leave the field,
    /// instead of letting Esc close the whole screen. False when no size
    /// field has focus.
    fn cancel_size_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(slot) = FontSlot::ALL.into_iter().find(|&slot| {
            self.fonts.sizes[slot_index(slot)]
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        }) else {
            return false;
        };
        let applied = cx.global::<themes::Themes>().fonts.size(slot);
        // Restore the text first: the blur that follows then has nothing
        // new to apply.
        self.fonts.texts[slot_index(slot)] = format_size(applied);
        self.set_size_input(slot, window, cx);
        window.blur(cx);
        cx.notify();
        true
    }

    /// Show the applied sizes in every field the user is not typing in.
    fn sync_size_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let applied = cx.global::<themes::Themes>().fonts.clone();
        for slot in FontSlot::ALL {
            let input = &self.fonts.sizes[slot_index(slot)];
            if input.read(cx).focus_handle(cx).is_focused(window) {
                continue;
            }
            self.fonts.texts[slot_index(slot)] = format_size(applied.size(slot));
            self.set_size_input(slot, window, cx);
        }
    }

    fn set_size_input(&mut self, slot: FontSlot, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.fonts.texts[slot_index(slot)].clone();
        self.fonts.sizes[slot_index(slot)].update(cx, |state, cx| {
            if state.value().as_ref() != text {
                state.set_value(text, window, cx);
            }
        });
    }

    /// What the Reading Desk draws, and whether it differs from the app.
    pub(super) fn desk_fonts(&self, cx: &App) -> (Fonts, bool) {
        let applied = &cx.global::<themes::Themes>().fonts;
        let fonts = &self.fonts;
        let peek = fonts.open.and_then(|slot| {
            fonts
                .entries
                .get(fonts.cursor)
                .map(|entry| (slot, entry.as_deref()))
        });
        let preview = preview_fonts(applied, peek, &fonts.texts);
        let previewing = preview != *applied;
        (preview, previewing)
    }

    /// The Fonts rows: a button per slot showing its family, opening its picker.
    pub(super) fn fonts_rows(&self, cx: &mut Context<Self>) -> Vec<Div> {
        FontSlot::ALL
            .into_iter()
            .map(|slot| {
                h_flex()
                    .pb_2()
                    .gap_3()
                    .child(
                        div()
                            .w(rems(120. / 16.))
                            .text_sm()
                            .text_color(ui::muted())
                            .child(slot_label(slot)),
                    )
                    .child(div().flex_1().min_w_0().child(self.font_picker(slot, cx)))
                    .child(self.size_field(slot))
            })
            .collect()
    }

    /// The size column: a number field for code and terminal. The interface
    /// has no size of its own (Interface size, the zoom, sets it), so its row
    /// keeps the column empty to line the pickers up.
    fn size_field(&self, slot: FontSlot) -> Div {
        let column = h_flex()
            .w(rems(72. / 16.))
            .flex_shrink_0()
            .gap_1()
            .items_center();
        if !slot.has_size() {
            return column;
        }
        let input = &self.fonts.sizes[slot_index(slot)];
        let invalid = parse_size(&self.fonts.texts[slot_index(slot)]) == SizeInput::Invalid;
        column
            .child(
                div()
                    .w(rems(48. / 16.))
                    .rounded_md()
                    .when(invalid, |d| d.border_1().border_color(ui::danger()))
                    .child(Input::new(input).small()),
            )
            .child(div().text_xs().text_color(ui::muted()).child("px"))
    }

    fn font_picker(&self, slot: FontSlot, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let open = self.fonts.open == Some(slot);
        let current = cx
            .global::<themes::Themes>()
            .fonts
            .get(slot)
            .map(|f| f.to_string());
        let label = current
            .clone()
            .unwrap_or_else(|| default_label(slot).to_string());
        let trigger = Button::new(SharedString::from(format!("font-{}", slot.key())))
            .small()
            .ghost()
            .label(label)
            .text_color(if current.is_some() {
                ui::fg()
            } else {
                ui::muted()
            })
            .dropdown_caret(true);
        // Share the cached entries with the popover without copying the list.
        let entries = if open {
            self.fonts.entries.clone()
        } else {
            Rc::from([])
        };
        let cursor = self.fonts.cursor;
        let search = self.fonts.search.clone();
        let scroll = self.fonts.scroll.clone();
        let toggle = weak.clone();

        Popover::new(SharedString::from(format!("font-menu-{}", slot.key())))
            .anchor(Anchor::TopLeft)
            .trigger(trigger)
            .open(open)
            .track_focus(&self.fonts.search.focus_handle(cx))
            .on_open_change(move |open, window, cx| {
                let slot = open.then_some(slot);
                let _ = toggle.update(cx, |this, cx| this.set_fonts_menu(slot, window, cx));
            })
            .w(rems(360. / 16.))
            .appearance(false)
            .content(move |_, _, cx| {
                let entries_for_list = entries.clone();
                let current_for_list = current.clone();
                let weak_for_list = weak.clone();
                let cursor_for_list = cursor;
                let slot_for_list = slot;

                let list = uniform_list(
                    "font-menu-rows",
                    entries.len(),
                    move |range: std::ops::Range<usize>, _, _| {
                        entries_for_list[range.start..range.end]
                            .iter()
                            .enumerate()
                            .map(|(offset, entry)| {
                                let index = range.start + offset;
                                let entry = entry.clone();
                                let chosen = entry == current_for_list;
                                let title: SharedString = match &entry {
                                    Some(name) => name.clone().into(),
                                    None => default_label(slot_for_list).into(),
                                };
                                let family = entry.clone();
                                let hover_weak = weak_for_list.clone();
                                let click_weak = weak_for_list.clone();
                                ui::picker_row(
                                    ("font-menu-row", index),
                                    index == cursor_for_list,
                                    chosen,
                                    None,
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(rems(13. / 16.))
                                        .text_color(match (chosen, &entry) {
                                            (true, _) => ui::accent(),
                                            (_, Some(_)) => ui::fg(),
                                            _ => ui::muted(),
                                        })
                                        .child(title),
                                )
                                .on_hover(move |hovered, _, cx| {
                                    if *hovered {
                                        let _ = hover_weak.update(cx, |this, cx| {
                                            if this.fonts.open == Some(slot_for_list)
                                                && this.fonts.cursor != index
                                            {
                                                this.fonts.cursor = index;
                                                cx.notify();
                                            }
                                        });
                                    }
                                })
                                .on_click(move |_, window, cx| {
                                    let family = family.clone();
                                    let _ = click_weak.update(cx, |this, cx| {
                                        this.choose_font(
                                            slot_for_list,
                                            family.as_deref(),
                                            window,
                                            cx,
                                        )
                                    });
                                })
                                .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .px(rems(6. / 16.))
                .pb(rems(6. / 16.))
                .track_scroll(&scroll);
                let list = fit_rows(list, rems(320. / 16.));

                let list = if entries.is_empty() || entries.as_ref() == [None] {
                    v_flex()
                        .id("font-menu-list")
                        .child(list)
                        .child(
                            div()
                                .px_3()
                                .py_1p5()
                                .text_xs()
                                .text_color(ui::faint())
                                .child(t("No installed font matches.")),
                        )
                        .into_any_element()
                } else {
                    list.into_any_element()
                };

                let content = v_flex()
                    .w_full()
                    .font_family(cx.theme().font_family.clone())
                    .text_color(ui::fg())
                    .child(ui::search_well(&search))
                    .child(list);
                ui::island(cx)
                    .w_full()
                    .child(ui::popover_in(
                        ElementId::Name("font-menu-in".into()),
                        content,
                    ))
                    .into_any_element()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Change, FontSlot, HashSet, SizeInput, effective_family, entry_cursor, fit_rows,
        format_size, parse_size, picker_entries, preview_fonts,
    };
    use crate::shared::themes::Fonts;
    use crate::shared::ui;
    // Not a glob: GPUI exports a `test` attribute that would shadow `#[test]`.
    use gpui_kit::component::v_flex;
    use gpui_kit::{
        Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render,
        Styled as _, TestAppContext, VisualTestContext, Window, anchored, div, px, rems,
        uniform_list,
    };

    fn texts(fonts: &Fonts) -> [String; 3] {
        FontSlot::ALL.map(|slot| format_size(fonts.size(slot)))
    }

    #[test]
    fn size_text_distinguishes_valid_intermediate_and_invalid() {
        assert_eq!(parse_size("14"), SizeInput::Valid(14.));
        assert_eq!(parse_size(" 8.5 "), SizeInput::Valid(8.5));
        assert_eq!(parse_size("8"), SizeInput::Valid(8.));
        assert_eq!(parse_size("32"), SizeInput::Valid(32.));
        for typing in ["", "  ", "1", "2", "3", "12."] {
            assert_eq!(parse_size(typing), SizeInput::Incomplete, "{typing:?}");
        }
        for bad in [
            "7", "33", "0", "99", "abc", "-14", "+14", "1e1", ".5", "1.2.3", "NaN", "inf", "14px",
        ] {
            assert_eq!(parse_size(bad), SizeInput::Invalid, "{bad:?}");
        }
    }

    #[test]
    fn nothing_hovered_or_typed_previews_the_applied_fonts() {
        let applied = Fonts::default();
        assert_eq!(preview_fonts(&applied, None, &texts(&applied)), applied);
    }

    #[test]
    fn a_hovered_family_previews_only_its_slot() {
        let applied = Fonts::default();
        let preview = preview_fonts(
            &applied,
            Some((FontSlot::Code, Some("IosevkaTerm"))),
            &texts(&applied),
        );
        assert_eq!(
            preview.get(FontSlot::Code).map(|f| f.as_ref()),
            Some("IosevkaTerm")
        );
        assert_eq!(preview.get(FontSlot::Interface), None);
        assert_eq!(preview.get(FontSlot::Terminal), None);
        assert_eq!(preview.size(FontSlot::Code), applied.size(FontSlot::Code));
        // Hovering the Default row of a chosen slot previews the default.
        let mut chosen = Fonts::default();
        chosen.set(FontSlot::Code, Some("Menlo".into()));
        let preview = preview_fonts(&chosen, Some((FontSlot::Code, None)), &texts(&chosen));
        assert_eq!(preview.get(FontSlot::Code), None);
    }

    #[test]
    fn only_a_valid_typed_size_previews() {
        let applied = Fonts::default();
        let mut typed = texts(&applied);
        typed[2] = "18".into();
        let preview = preview_fonts(&applied, None, &typed);
        assert_eq!(preview.size(FontSlot::Terminal), 18.);
        assert_eq!(preview.size(FontSlot::Code), applied.size(FontSlot::Code));
        for unfinished in ["", "1", "99", "abc"] {
            typed[2] = unfinished.into();
            let preview = preview_fonts(&applied, None, &typed);
            assert_eq!(
                preview.size(FontSlot::Terminal),
                applied.size(FontSlot::Terminal)
            );
        }
    }

    #[test]
    fn undo_puts_back_only_the_changed_slot() {
        let mut fonts = Fonts::default();
        fonts.set(FontSlot::Code, Some("IosevkaTerm".into()));
        let code_change = Change::Family(FontSlot::Code, None);
        // A later, unrelated change survives undoing the earlier one.
        fonts.set_size(FontSlot::Terminal, 18.);
        let undone = code_change.revert(&fonts);
        assert_eq!(undone.get(FontSlot::Code), None);
        assert_eq!(undone.size(FontSlot::Terminal), 18.);
        let size_change = Change::Size(FontSlot::Terminal, 13.);
        let undone = size_change.revert(&undone);
        assert_eq!(undone, Fonts::default());
    }

    #[test]
    fn the_toast_names_what_was_applied() {
        let mut fonts = Fonts::default();
        fonts.set(FontSlot::Code, Some("IosevkaTerm".into()));
        fonts.set_size(FontSlot::Terminal, 15.5);
        assert_eq!(
            Change::Family(FontSlot::Code, None).message(&fonts),
            "Code font changed to IosevkaTerm. Click to undo."
        );
        assert_eq!(
            Change::Family(FontSlot::Interface, Some("Georgia".into())).message(&fonts),
            "Interface font reset to default. Click to undo."
        );
        assert_eq!(
            Change::Family(FontSlot::Terminal, Some("Menlo".into())).message(&fonts),
            "Terminal font now follows the code font. Click to undo."
        );
        assert_eq!(
            Change::Size(FontSlot::Terminal, 13.).message(&fonts),
            "Terminal font size changed to 15.5 px. Click to undo."
        );
    }

    #[test]
    fn terminal_preview_inherits_only_the_code_family() {
        let interface = "Kit Interface".into();
        let mut fonts = Fonts::default();
        assert_eq!(
            effective_family(&fonts, FontSlot::Interface, &interface),
            interface
        );
        assert_eq!(
            effective_family(&fonts, FontSlot::Code, &interface),
            ui::default_mono()
        );
        assert_eq!(
            effective_family(&fonts, FontSlot::Terminal, &interface),
            ui::default_mono()
        );
        fonts.set(FontSlot::Code, Some("Candidate".into()));
        fonts.set_size(FontSlot::Code, 20.);
        assert_eq!(
            effective_family(&fonts, FontSlot::Terminal, &interface),
            "Candidate"
        );
        assert_ne!(fonts.size(FontSlot::Terminal), 20.);
        fonts.set(FontSlot::Terminal, Some("Own".into()));
        assert_eq!(
            effective_family(&fonts, FontSlot::Terminal, &interface),
            "Own"
        );
    }

    #[test]
    fn metadata_promotes_monospace_without_moving_selection_to_another_family() {
        let names = ["Menlo".to_string(), "Arial".to_string()];
        let before = picker_entries(&names, "", None);
        assert_eq!(before, [None, Some("Arial".into()), Some("Menlo".into())]);

        let monospace = HashSet::from(["menlo".to_string()]);
        let after = picker_entries(&names, "", Some(&monospace));
        assert_eq!(after, [None, Some("Menlo".into()), Some("Arial".into())]);
        assert_eq!(entry_cursor(&after, before.get(1)), 2);
        assert_eq!(entry_cursor(&after, before.first()), 0);
    }

    /// A popover-like host: a content-sized column, positioned like the
    /// popover, with a header and the font list of `rows` 28 px rows.
    struct ListHost(usize);

    impl Render for ListHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let list = uniform_list("rows", self.0, |range, _, _| {
                range
                    .map(|i| div().h(px(28.)).child(format!("Font {i}")))
                    .collect::<Vec<_>>()
            })
            .pb(px(6.));
            div().size_full().child(
                anchored().child(
                    v_flex()
                        .w(px(360.))
                        .debug_selector(|| "popover".into())
                        .child(div().h(px(30.)))
                        .child(fit_rows(list, rems(20.)).debug_selector(|| "list".into())),
                ),
            )
        }
    }

    fn heights(rows: usize, cx: &mut TestAppContext) -> (Pixels, Pixels) {
        let (_, cx) = cx.add_window_view(|_, _| ListHost(rows));
        cx.run_until_parked();
        let height = |cx: &mut VisualTestContext, selector| {
            cx.debug_bounds(selector).expect("drawn").size.height
        };
        (height(cx, "list"), height(cx, "popover"))
    }

    #[gpui_kit::test]
    fn the_font_list_fits_its_rows_up_to_the_cap(cx: &mut TestAppContext) {
        // Two rows: the list shows both, and the popover hugs them.
        assert_eq!(heights(2, cx), (px(62.), px(92.)));
        // Many rows: the list stops at 320 px and so does the popover,
        // instead of growing to every row.
        assert_eq!(heights(200, cx), (px(320.), px(350.)));
    }

    #[test]
    fn metadata_respects_the_active_search() {
        let names = ["Arial".to_string(), "Menlo".to_string()];
        let monospace = HashSet::from(["menlo".to_string()]);
        let rows = picker_entries(&names, "ari", Some(&monospace));
        assert_eq!(rows, [Some("Arial".into())]);
        assert!(picker_entries(&names, "zzz", Some(&monospace)).is_empty());
    }
}

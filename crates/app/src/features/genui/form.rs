//! The form panel: the agent's questions as native controls, validated
//! here before Submit sends every answer as one change. A submitted form
//! shows its answers read-only until the agent updates it.

use std::collections::HashMap;

use gentle_core::genui::edits::Edits;
use gentle_core::genui::form::{Change, Field, FieldType, Form};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Map, Value, json};

use super::diagram::SendState;
use super::panel::{Body, CHROME, MAX_HEIGHT, PanelView};
use crate::shared::ui;

/// A text field's state: one line, or several.
pub(super) enum TextField {
    Line(Entity<InputState>),
    Area(Entity<TextareaState>),
}

impl TextField {
    fn value(&self, cx: &App) -> String {
        match self {
            TextField::Line(input) => input.read(cx).value().to_string(),
            TextField::Area(input) => input.read(cx).value().to_string(),
        }
    }
}

pub(super) struct FormState {
    pub edits: Edits<Form>,
    pub uses_default_submit_label: bool,
    /// Text, textarea, number and date fields (made at first draw: they
    /// need the window).
    pub texts: HashMap<String, TextField>,
    pub sliders: HashMap<String, Entity<SliderState>>,
    /// Checkbox, select, multiselect and radio answers.
    pub choices: Map<String, Value>,
    /// The last Submit's problems, by field id.
    pub errors: Vec<(String, gentle_core::genui::form::AnswerError)>,
    subscriptions: Vec<Subscription>,
}

impl FormState {
    pub fn new(form: Form, spec: &Value) -> Self {
        let mut state = Self {
            edits: Edits::new(form),
            uses_default_submit_label: Form::custom_submit_label(spec.get("submitLabel")).is_none(),
            texts: HashMap::new(),
            sliders: HashMap::new(),
            choices: Map::new(),
            errors: Vec::new(),
            subscriptions: Vec::new(),
        };
        state.load_choices();
        state
    }

    fn load_choices(&mut self) {
        let form = self.edits.working();
        for field in &form.fields {
            let choice = matches!(
                field.kind,
                FieldType::Checkbox | FieldType::Select | FieldType::Multiselect | FieldType::Radio
            );
            if choice && !self.choices.contains_key(&field.id) {
                self.choices.insert(field.id.clone(), form.value(&field.id));
            }
        }
    }

    /// A newer version: answers typed so far stay for the fields that are
    /// still there; new fields start from the agent's values.
    pub fn rebase(&mut self, form: Form, spec: &Value) {
        self.uses_default_submit_label =
            Form::custom_submit_label(spec.get("submitLabel")).is_none();
        let reopened = self.edits.base().submitted && !form.submitted;
        self.edits.rebase(form);
        let form = self.edits.working();
        if reopened {
            // Asked again: start from the agent's values.
            self.texts.clear();
            self.sliders.clear();
            self.choices.clear();
        }
        self.texts.retain(|id, _| form.field(id).is_some());
        self.sliders.retain(|id, _| form.field(id).is_some());
        self.choices.retain(|id, _| form.field(id).is_some());
        self.errors.clear();
        self.load_choices();
    }

    pub fn height(&self) -> f32 {
        let form = self.edits.working();
        let mut h = 24. + if form.description.is_some() { 30. } else { 0. };
        for field in &form.fields {
            h += 12.
                + 20.
                + if field.help.is_some() { 16. } else { 0. }
                + match field.kind {
                    _ if form.submitted => 0.,
                    FieldType::Textarea => 76.,
                    FieldType::Radio => 26. * field.options.len() as f32,
                    FieldType::Checkbox => 0.,
                    FieldType::Select | FieldType::Multiselect => 34.,
                    _ => 36.,
                };
        }
        h.clamp(140., MAX_HEIGHT - CHROME)
    }

    /// Every field's answer as typed or chosen now.
    fn answers(&self, cx: &App) -> Map<String, Value> {
        let form = self.edits.working();
        let mut values = Map::new();
        for field in &form.fields {
            let value = if let Some(text) = self.texts.get(&field.id) {
                json!(text.value(cx))
            } else if let Some(slider) = self.sliders.get(&field.id) {
                json!(f64::from(slider.read(cx).value().start()))
            } else {
                self.choices
                    .get(&field.id)
                    .cloned()
                    .unwrap_or_else(|| form.value(&field.id))
            };
            values.insert(field.id.clone(), value);
        }
        values
    }
}

fn state(this: &mut PanelView) -> Option<&mut FormState> {
    match &mut this.body {
        Some(Body::Form(s)) => Some(s),
        _ => None,
    }
}

/// A field's answer as text for an input.
fn text_of(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl PanelView {
    fn form(&self) -> Option<&FormState> {
        match &self.body {
            Some(Body::Form(s)) => Some(s),
            _ => None,
        }
    }

    fn ensure_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.form() else { return };
        let form = s.edits.working();
        let missing: Vec<Field> = form
            .fields
            .iter()
            .filter(|f| !s.texts.contains_key(&f.id) && !s.sliders.contains_key(&f.id))
            .filter(|f| {
                matches!(
                    f.kind,
                    FieldType::Text
                        | FieldType::Textarea
                        | FieldType::Number
                        | FieldType::Date
                        | FieldType::Slider
                )
            })
            .cloned()
            .collect();
        if missing.is_empty() {
            return;
        }
        let values: Vec<Value> = missing.iter().map(|f| form.value(&f.id)).collect();
        for (field, value) in missing.into_iter().zip(values) {
            let placeholder = field
                .placeholder
                .clone()
                .unwrap_or_else(|| match field.kind {
                    FieldType::Date => "YYYY-MM-DD".into(),
                    FieldType::Number => "0".into(),
                    _ => String::new(),
                });
            match field.kind {
                FieldType::Slider => {
                    let (min, max) = (field.min.unwrap_or(0.), field.max.unwrap_or(100.));
                    let start = value.as_f64().unwrap_or(min);
                    let slider = cx.new(|_| {
                        SliderState::new()
                            .min(min as f32)
                            .max(max as f32)
                            .step(field.step.unwrap_or(((max - min) / 100.).max(0.01)) as f32)
                            .default_value(start as f32)
                    });
                    let subscription =
                        cx.subscribe(&slider, |_, _, _: &SliderEvent, cx| cx.notify());
                    if let Some(s) = state(self) {
                        s.sliders.insert(field.id.clone(), slider);
                        s.subscriptions.push(subscription);
                    }
                }
                FieldType::Textarea => {
                    let text = text_of(&value);
                    let input = cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .auto_grow(3, 3)
                            .placeholder(placeholder)
                            .default_value(text)
                    });
                    if let Some(s) = state(self) {
                        s.texts.insert(field.id.clone(), TextField::Area(input));
                    }
                }
                _ => {
                    let text = text_of(&value);
                    let input = cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(placeholder)
                            .default_value(text)
                    });
                    // Enter in a one-line field submits, as in a web form.
                    let subscription = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                        if let InputEvent::PressEnter { .. } = event {
                            this.submit_form(cx);
                        }
                    });
                    if let Some(s) = state(self) {
                        s.texts.insert(field.id.clone(), TextField::Line(input));
                        s.subscriptions.push(subscription);
                    }
                }
            }
        }
    }

    fn set_choice(&mut self, field: &str, value: Value, cx: &mut Context<Self>) {
        if let Some(s) = state(self) {
            s.choices.insert(field.to_string(), value);
            s.errors.retain(|(id, _)| id != field);
        }
        cx.notify();
    }

    fn toggle_option(&mut self, field: &str, option: &str, cx: &mut Context<Self>) {
        let Some(s) = self.form() else { return };
        let mut chosen: Vec<Value> = s
            .choices
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        match chosen.iter().position(|v| v == option) {
            Some(i) => {
                chosen.remove(i);
            }
            None => chosen.push(json!(option)),
        }
        self.set_choice(field, Value::Array(chosen), cx);
    }

    /// Check every answer; when they pass, submit them as one change and
    /// send it.
    pub(super) fn submit_form(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.form() else { return };
        let form = s.edits.working();
        if form.submitted {
            return;
        }
        let values = s.answers(cx);
        if let Err(errors) = form.validate_answers(&values) {
            if let Some(s) = state(self) {
                s.errors = errors;
            }
            cx.notify();
            return;
        }
        if let Some(s) = state(self) {
            s.errors.clear();
        }
        let change = Change::Submit { values };
        let applied = self.edit_with(cx, |body| match body {
            Body::Form(s) => Some(s.edits.edit(&change)),
            _ => None,
        });
        if applied {
            self.send(cx);
        }
    }

    pub(super) fn render_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.ensure_fields(window, cx);
        let Some(s) = self.form() else {
            return div().into_any_element();
        };
        let form = s.edits.working();
        let fields: Vec<AnyElement> = if form.submitted {
            form.fields
                .iter()
                .map(|f| answer_row(f, &form.value(&f.id)))
                .collect()
        } else {
            form.fields
                .iter()
                .map(|f| self.render_field(f, cx))
                .collect()
        };
        v_flex()
            .id("genui-form")
            .size_full()
            .overflow_y_scroll()
            .p_3()
            .gap_3()
            .when_some(form.description.clone(), |d, description| {
                d.child(
                    div()
                        .text_size(rems(12.5 / 16.))
                        .text_color(ui::muted())
                        .whitespace_normal()
                        .child(description),
                )
            })
            .children(fields)
            .into_any_element()
    }

    fn render_field(&self, field: &Field, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.form() else {
            return div().into_any_element();
        };
        let error = s
            .errors
            .iter()
            .find(|(id, _)| *id == field.id)
            .map(|(_, message)| message.render(crate::shared::i18n::language()));
        let choice = s.choices.get(&field.id).cloned().unwrap_or(Value::Null);
        let selector = format!("genui-field-{}", field.id);
        let control: AnyElement = match field.kind {
            FieldType::Text | FieldType::Number | FieldType::Date => match s.texts.get(&field.id) {
                Some(TextField::Line(input)) => ui::field()
                    .debug_selector({
                        let selector = format!("genui-input-{}", field.id);
                        move || selector
                    })
                    .h(rems(32. / 16.))
                    .when(error.is_some(), |d| {
                        d.border_color(ui::danger().opacity(0.6))
                    })
                    .when(field.kind != FieldType::Text, |d| d.font_family(ui::mono()))
                    .child(Input::new(input).appearance(false).small())
                    .into_any_element(),
                _ => div().into_any_element(),
            },
            FieldType::Textarea => match s.texts.get(&field.id) {
                Some(TextField::Area(input)) => ui::field()
                    .h_auto()
                    .py_1p5()
                    .items_start()
                    .when(error.is_some(), |d| {
                        d.border_color(ui::danger().opacity(0.6))
                    })
                    .child(Textarea::new(input).appearance(false))
                    .into_any_element(),
                _ => div().into_any_element(),
            },
            FieldType::Slider => match s.sliders.get(&field.id) {
                Some(slider) => {
                    let value = f64::from(slider.read(cx).value().start());
                    h_flex()
                        .gap_3()
                        .child(div().flex_1().child(Slider::new(slider).horizontal()))
                        .child(
                            div()
                                .min_w(rems(40. / 16.))
                                .font_family(ui::mono())
                                .text_size(ui::code_rems(12.))
                                .text_color(ui::fg())
                                .child(field.display(&json!(value))),
                        )
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
            FieldType::Checkbox => {
                let id = field.id.clone();
                let checked = choice.as_bool().unwrap_or(false);
                return v_flex()
                    .gap_1()
                    .child(
                        h_flex().child(
                            div().debug_selector(move || selector).child(
                                Checkbox::new(SharedString::from(format!(
                                    "genui-field-{}",
                                    field.id
                                )))
                                .small()
                                .checked(checked)
                                .label(field.label.clone())
                                .on_click(cx.listener(
                                    move |this, checked: &bool, _, cx| {
                                        this.set_choice(&id, json!(*checked), cx)
                                    },
                                )),
                            ),
                        ),
                    )
                    .children(field.help.clone().map(help))
                    .children(error.map(problem))
                    .into_any_element();
            }
            FieldType::Radio => {
                v_flex()
                    .gap_0p5()
                    .children(field.options.iter().enumerate().map(|(i, option)| {
                        let on = choice.as_str() == Some(option.value.as_str());
                        let (id, value) = (field.id.clone(), option.value.clone());
                        let selector = format!("genui-option-{}-{}", field.id, option.value);
                        h_flex()
                            .id((SharedString::from(format!("genui-radio-{}", field.id)), i))
                            .debug_selector(move || selector)
                            .gap_2()
                            .h(rems(24. / 16.))
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|d| d.bg(ui::surface_hover()))
                            .child(
                                Icon::new(if on {
                                    IconName::CircleCheck
                                } else {
                                    IconName::Circle
                                })
                                .small()
                                .text_color(if on {
                                    ui::accent()
                                } else {
                                    ui::faint()
                                }),
                            )
                            .child(
                                div()
                                    .text_size(rems(12.5 / 16.))
                                    .text_color(if on { ui::fg() } else { ui::muted() })
                                    .child(option.label.clone()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_choice(&id, json!(value), cx)
                            }))
                    }))
                    .into_any_element()
            }
            FieldType::Select | FieldType::Multiselect => {
                let multiple = field.kind == FieldType::Multiselect;
                h_flex()
                    .flex_wrap()
                    .gap_1p5()
                    .children(field.options.iter().enumerate().map(|(i, option)| {
                        let on = if multiple {
                            choice
                                .as_array()
                                .is_some_and(|c| c.iter().any(|v| v == option.value.as_str()))
                        } else {
                            choice.as_str() == Some(option.value.as_str())
                        };
                        let (id, value) = (field.id.clone(), option.value.clone());
                        let selector = format!("genui-option-{}-{}", field.id, option.value);
                        h_flex()
                            .id((SharedString::from(format!("genui-pill-{}", field.id)), i))
                            .debug_selector(move || selector)
                            .gap_1()
                            .px_2p5()
                            .h(rems(26. / 16.))
                            .rounded_lg()
                            .border_1()
                            .cursor_pointer()
                            .text_size(rems(12. / 16.))
                            .when(on, |d| {
                                d.bg(ui::accent().opacity(0.14))
                                    .border_color(ui::accent().opacity(0.5))
                                    .text_color(ui::fg())
                            })
                            .when(!on, |d| {
                                d.border_color(ui::border_strong())
                                    .text_color(ui::muted())
                                    .hover(|d| d.text_color(ui::fg()).bg(ui::surface_hover()))
                            })
                            .when(multiple && on, |d| {
                                d.child(
                                    Icon::new(IconName::Check).xsmall().text_color(ui::accent()),
                                )
                            })
                            .child(option.label.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if multiple {
                                    this.toggle_option(&id, &value, cx)
                                } else {
                                    this.set_choice(&id, json!(value), cx)
                                }
                            }))
                    }))
                    .into_any_element()
            }
        };
        v_flex()
            .gap_1()
            .debug_selector(move || selector)
            .child(
                h_flex()
                    .gap_1()
                    .text_size(rems(12.5 / 16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ui::fg())
                    .child(field.label.clone())
                    .when(field.required, |d| {
                        d.child(div().text_color(ui::danger()).child("*"))
                    }),
            )
            .children(field.help.clone().map(help))
            .child(control)
            .children(error.map(problem))
            .into_any_element()
    }

    /// The form's footer: Submit (with the problems of the last try), the
    /// submission on its way, or the answers sent.
    pub(super) fn form_footer(&self, form: &FormState, footer: Div, cx: &mut Context<Self>) -> Div {
        let working = form.edits.working();
        if form.edits.base().submitted && form.edits.pending() == 0 {
            return footer
                .text_color(ui::success())
                .child(Icon::new(IconName::CircleCheck).xsmall())
                .child(
                    div()
                        .flex_1()
                        .child(crate::shared::i18n::t("Submitted to the agent")),
                )
                .child(div().text_color(ui::faint()).child(crate::shared::i18n::t(
                    "It can reopen the form to ask again",
                )));
        }
        if working.submitted {
            // Sent and not answered yet, or taken by an extension that doesn't
            // answer (its echo makes the answers the base).
            let sending = matches!(self.send, SendState::Sending { .. } | SendState::Sent);
            return footer
                .when(sending, |d| {
                    d.text_color(ui::muted())
                        .child(ui::spinner(gpui_kit::component::Size::XSmall, ui::info()))
                        .child(div().flex_1().child(crate::shared::i18n::t("Submitting…")))
                })
                .when(!sending, |d| {
                    let error = match &self.send {
                        SendState::Failed(error) => error.clone(),
                        _ => crate::shared::i18n::t("Not sent yet").into(),
                    };
                    d.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(ui::danger())
                            .child(error),
                    )
                    .child(
                        Button::new("genui-form-edit")
                            .ghost()
                            .xsmall()
                            .label(crate::shared::i18n::t("Edit answers"))
                            .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
                    )
                    .child(
                        Button::new("genui-form-retry")
                            .primary()
                            .xsmall()
                            .icon(Icon::new(IconName::Send))
                            .label(crate::shared::i18n::t("Retry"))
                            .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
                    )
                });
        }
        let problems = form.errors.len();
        footer
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(problems > 0, |d| {
                        d.text_color(ui::danger()).child(if problems == 1 {
                            crate::shared::i18n::t("1 answer needs a fix").to_string()
                        } else {
                            crate::shared::i18n::t("{count} answers need a fix")
                                .replace("{count}", &problems.to_string())
                        })
                    })
                    .when(problems == 0, |d| {
                        d.text_color(ui::faint()).child(crate::shared::i18n::t(
                            "The agent gets every answer at once",
                        ))
                    }),
            )
            .child(
                div().debug_selector(|| "genui-form-submit".into()).child(
                    Button::new("genui-form-submit")
                        .primary()
                        .xsmall()
                        .icon(Icon::new(IconName::Send))
                        .label(if form.uses_default_submit_label {
                            crate::shared::i18n::t("Submit").to_owned()
                        } else {
                            working.submit_label.clone()
                        })
                        .disabled(matches!(self.send, SendState::Sending { .. }))
                        .on_click(cx.listener(|this, _, _, cx| this.submit_form(cx))),
                ),
            )
    }
}

fn help(text: String) -> Div {
    div()
        .text_size(rems(11.5 / 16.))
        .text_color(ui::muted())
        .whitespace_normal()
        .child(text)
}

fn problem(text: String) -> Div {
    h_flex()
        .gap_1()
        .text_size(rems(11.5 / 16.))
        .text_color(ui::danger())
        .child(Icon::new(IconName::CircleAlert).xsmall())
        .child(text)
}

/// A submitted answer, read-only.
fn answer_row(field: &Field, value: &Value) -> AnyElement {
    h_flex()
        .items_start()
        .gap_3()
        .text_size(rems(12.5 / 16.))
        .child(
            div()
                .w(rems(140. / 16.))
                .flex_shrink_0()
                .text_color(ui::muted())
                .truncate()
                .child(field.label.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(ui::fg())
                .whitespace_normal()
                .child(field.display(value)),
        )
        .into_any_element()
}

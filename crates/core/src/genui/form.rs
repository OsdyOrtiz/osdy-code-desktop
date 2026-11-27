//! The form kind: fields the agent asks the user to fill in. Submit sends
//! every value as one change; a submitted form shows read-only until the
//! agent updates it (which reopens it).

use serde_json::{Map, Value, json};

use super::diagram::{Errors, MAX_LABEL, MAX_TEXT, array, id_of, text, truncate};
use super::edits::{Editable, format_number, number};

pub const MAX_FIELDS: usize = 50;
pub const MAX_OPTIONS: usize = 50;
/// A text value is cut to this many characters (textarea: [`MAX_TEXT`] × 4).
pub const MAX_VALUE: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldType {
    #[default]
    Text,
    Textarea,
    Number,
    Select,
    Multiselect,
    Checkbox,
    Radio,
    Slider,
    Date,
}

impl FieldType {
    pub const ALL: [Self; 9] = [
        Self::Text,
        Self::Textarea,
        Self::Number,
        Self::Select,
        Self::Multiselect,
        Self::Checkbox,
        Self::Radio,
        Self::Slider,
        Self::Date,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Textarea => "textarea",
            Self::Number => "number",
            Self::Select => "select",
            Self::Multiselect => "multiselect",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Slider => "slider",
            Self::Date => "date",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(s.trim()))
    }

    pub fn has_options(self) -> bool {
        matches!(self, Self::Select | Self::Multiselect | Self::Radio)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub id: String,
    pub kind: FieldType,
    pub label: String,
    pub help: Option<String>,
    pub placeholder: Option<String>,
    pub required: bool,
    pub options: Vec<Choice>,
    /// Numbers: the value's bounds; text: its length; multiselect: how
    /// many; dates: ISO dates as text in `min_date`/`max_date`.
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
    pub step: Option<f64>,
    /// Valid for the field, or `Null`.
    pub default: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    pub description: Option<String>,
    pub submit_label: String,
    pub fields: Vec<Field>,
    /// The answers (the agent's prefill, or what the user submitted).
    pub values: Map<String, Value>,
    pub submitted: bool,
}

/// `YYYY-MM-DD` with a real month and day.
pub fn is_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return false;
    };
    let all_digits = |p: &str, n: usize| p.len() == n && p.bytes().all(|b| b.is_ascii_digit());
    if !(all_digits(y, 4) && all_digits(m, 2) && all_digits(d, 2)) {
        return false;
    }
    let (y, m, d): (u32, u32, u32) = (y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap());
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

/// An answer validation diagnostic, rendered in the current UI language.
#[derive(Clone, Debug)]
pub struct AnswerError {
    template: &'static str,
    label: String,
    value: String,
}

impl AnswerError {
    pub fn render(&self, language: crate::i18n::Language) -> String {
        let template = crate::i18n::tr(language, self.template);
        let (before, after) = template
            .split_once("{label}")
            .expect("answer errors name their field");
        format!(
            "{}{}{}",
            before.replace("{value}", &self.value),
            self.label,
            after.replace("{value}", &self.value)
        )
    }
}

impl Field {
    pub fn option_label<'a>(&'a self, value: &'a str) -> &'a str {
        self.options
            .iter()
            .find(|o| o.value == value)
            .map_or(value, |o| o.label.as_str())
    }

    fn max_len(&self) -> usize {
        if self.kind == FieldType::Textarea {
            MAX_TEXT * 4
        } else {
            MAX_VALUE
        }
    }

    /// What an untouched field holds: the default, else empty (unchecked,
    /// nothing chosen, a slider at its minimum).
    pub fn empty_value(&self) -> Value {
        if !self.default.is_null() {
            return self.default.clone();
        }
        match self.kind {
            FieldType::Checkbox => json!(false),
            FieldType::Multiselect => json!([]),
            FieldType::Slider => json!(self.min.unwrap_or(0.)),
            _ => Value::Null,
        }
    }

    /// `value` in this field's shape, or why it doesn't fit (ignoring
    /// `required`). Numbers may come as text.
    pub fn coerce(&self, value: &Value) -> Result<Value, String> {
        self.coerce_answer(value)
            .map_err(|error| error.render(crate::i18n::Language::English))
    }

    fn answer_error(&self, template: &'static str, value: String) -> AnswerError {
        AnswerError {
            template,
            label: self.label.clone(),
            value,
        }
    }

    fn coerce_answer(&self, value: &Value) -> Result<Value, AnswerError> {
        let plain = |template| self.answer_error(template, String::new());
        if value.is_null() || value.as_str().is_some_and(|s| s.trim().is_empty()) {
            return Ok(match self.kind {
                FieldType::Checkbox => json!(false),
                FieldType::Multiselect => json!([]),
                _ => Value::Null,
            });
        }
        match self.kind {
            FieldType::Text | FieldType::Textarea => {
                let s = match value {
                    Value::String(s) => s.trim().to_string(),
                    Value::Number(_) | Value::Bool(_) => value.to_string(),
                    _ => return Err(plain("{label} must be text")),
                };
                let chars = s.chars().count() as f64;
                if let Some(min) = self.min.filter(|min| chars < *min) {
                    return Err(self.answer_error(
                        "{label} needs at least {value} characters",
                        min.to_string(),
                    ));
                }
                if let Some(max) = self.max.filter(|max| chars > *max) {
                    return Err(self.answer_error(
                        "{label} takes at most {value} characters",
                        max.to_string(),
                    ));
                }
                Ok(json!(truncate(&s, self.max_len())))
            }
            FieldType::Number | FieldType::Slider => {
                let n = number(value).ok_or_else(|| plain("{label} must be a number"))?;
                if let Some(min) = self.min.filter(|min| n < *min) {
                    return Err(
                        self.answer_error("{label} must be at least {value}", format_number(min))
                    );
                }
                if let Some(max) = self.max.filter(|max| n > *max) {
                    return Err(
                        self.answer_error("{label} must be at most {value}", format_number(max))
                    );
                }
                Ok(json!(n))
            }
            FieldType::Checkbox => match value {
                Value::Bool(_) => Ok(value.clone()),
                _ => Err(plain("{label} must be true or false")),
            },
            FieldType::Select | FieldType::Radio => {
                let s = value
                    .as_str()
                    .ok_or_else(|| plain("{label} must be one of its options"))?;
                if self.options.iter().any(|o| o.value == s) {
                    Ok(json!(s))
                } else {
                    Err(self.answer_error("“{value}” is not an option of {label}", s.to_owned()))
                }
            }
            FieldType::Multiselect => {
                let items = value
                    .as_array()
                    .ok_or_else(|| plain("{label} must be a list of options"))?;
                let mut chosen = Vec::new();
                for item in items {
                    let s = item.as_str().unwrap_or_default();
                    if !self.options.iter().any(|o| o.value == s) {
                        return Err(self
                            .answer_error("“{value}” is not an option of {label}", s.to_owned()));
                    }
                    if !chosen.contains(&s) {
                        chosen.push(s);
                    }
                }
                // Options order, so the answer doesn't depend on click order.
                let chosen: Vec<&str> = self
                    .options
                    .iter()
                    .filter(|o| chosen.contains(&o.value.as_str()))
                    .map(|o| o.value.as_str())
                    .collect();
                let count = chosen.len() as f64;
                if let Some(min) = self.min.filter(|min| count < *min) {
                    return Err(
                        self.answer_error("{label}: choose at least {value}", min.to_string())
                    );
                }
                if let Some(max) = self.max.filter(|max| count > *max) {
                    return Err(
                        self.answer_error("{label}: choose at most {value}", max.to_string())
                    );
                }
                Ok(json!(chosen))
            }
            FieldType::Date => {
                let s = value.as_str().map(str::trim).unwrap_or_default();
                if !is_date(s) {
                    return Err(plain("{label} must be a date (YYYY-MM-DD)"));
                }
                if let Some(min) = self.min_date.as_deref().filter(|min| s < *min) {
                    return Err(
                        self.answer_error("{label} must be on or after {value}", min.to_owned())
                    );
                }
                if let Some(max) = self.max_date.as_deref().filter(|max| s > *max) {
                    return Err(
                        self.answer_error("{label} must be on or before {value}", max.to_owned())
                    );
                }
                Ok(json!(s))
            }
        }
    }

    fn is_empty(value: &Value) -> bool {
        match value {
            Value::Null => true,
            Value::Bool(b) => !b,
            Value::Array(items) => items.is_empty(),
            Value::String(s) => s.trim().is_empty(),
            _ => false,
        }
    }

    /// A value for people: option labels, "yes"/"no", numbers without ".0".
    pub fn display(&self, value: &Value) -> String {
        match value {
            Value::Null => "—".into(),
            Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
            Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), format_number),
            Value::String(s) if self.kind.has_options() => self.option_label(s).to_string(),
            Value::String(s) => s.clone(),
            Value::Array(items) if items.is_empty() => "—".into(),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .map(|s| self.option_label(s))
                .collect::<Vec<_>>()
                .join(", "),
            Value::Object(_) => value.to_string(),
        }
    }
}

impl Form {
    pub fn parse(spec: &Value) -> Result<Form, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(fields_raw) = spec.get("fields").and_then(Value::as_array) else {
            return Err(vec!["\"fields\" must be an array".into()]);
        };
        let mut errors = Errors::default();
        if fields_raw.is_empty() {
            errors.push("a form needs at least one field".into());
        }
        if fields_raw.len() > MAX_FIELDS {
            errors.push(format!(
                "too many fields ({}, at most {MAX_FIELDS})",
                fields_raw.len()
            ));
        }
        let mut fields: Vec<Field> = Vec::new();
        for (i, raw) in fields_raw.iter().enumerate().take(MAX_FIELDS) {
            let Some(id) = raw.get("id").and_then(id_of) else {
                errors.push(format!("field #{} has no id", i + 1));
                continue;
            };
            if fields.iter().any(|f| f.id == id) {
                errors.push(format!("duplicate field id \"{id}\""));
                continue;
            }
            let kind = match raw.get("type") {
                None | Some(Value::Null) => FieldType::Text,
                Some(t) => match t.as_str().and_then(FieldType::parse) {
                    Some(kind) => kind,
                    None => {
                        errors.push(format!(
                            "field \"{id}\": unknown type {t} (one of {})",
                            FieldType::ALL.map(FieldType::name).join(", ")
                        ));
                        continue;
                    }
                },
            };
            let options: Vec<Choice> = array(raw.get("options"))
                .iter()
                .take(MAX_OPTIONS)
                .filter_map(|o| {
                    let value = match o {
                        Value::Object(_) => o.get("value").and_then(id_of),
                        _ => id_of(o),
                    }?;
                    let label = text(o.get("label"), MAX_LABEL).unwrap_or_else(|| value.clone());
                    Some(Choice { value, label })
                })
                .collect();
            if kind.has_options() && options.is_empty() {
                errors.push(format!("field \"{id}\" ({}) needs options", kind.name()));
            }
            let bound = |key: &str| raw.get(key).and_then(number);
            let date = |key: &str| {
                raw.get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| is_date(s))
                    .map(str::to_string)
            };
            let (mut min, mut max) = (bound("min"), bound("max"));
            if kind == FieldType::Slider {
                min = min.or(Some(0.));
                max = max.or(Some(100.));
            }
            if let (Some(lo), Some(hi)) = (min, max)
                && lo > hi
            {
                errors.push(format!("field \"{id}\": min is greater than max"));
            }
            let mut field = Field {
                label: text(raw.get("label"), MAX_LABEL).unwrap_or_else(|| id.clone()),
                help: text(raw.get("help"), MAX_TEXT),
                placeholder: text(raw.get("placeholder"), MAX_LABEL),
                required: raw.get("required").and_then(Value::as_bool) == Some(true),
                options,
                min: if kind == FieldType::Date { None } else { min },
                max: if kind == FieldType::Date { None } else { max },
                min_date: date("min"),
                max_date: date("max"),
                step: bound("step").filter(|s| *s > 0.),
                default: Value::Null,
                kind,
                id,
            };
            // A default that doesn't fit is dropped rather than rejected.
            field.default = raw
                .get("default")
                .and_then(|d| field.coerce(d).ok())
                .filter(|d| !Field::is_empty(d) || d.is_boolean())
                .unwrap_or(Value::Null);
            fields.push(field);
        }
        let mut values = Map::new();
        if let Some(raw) = spec.get("values").and_then(Value::as_object) {
            for field in &fields {
                if let Some(value) = raw.get(&field.id).and_then(|v| field.coerce(v).ok()) {
                    values.insert(field.id.clone(), value);
                }
            }
        }
        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(Form {
            description: text(spec.get("description"), MAX_TEXT),
            submit_label: Self::custom_submit_label(spec.get("submitLabel"))
                .unwrap_or_else(|| "Submit".into()),
            fields,
            values,
            submitted: spec.get("submitted").and_then(Value::as_bool) == Some(true),
        })
    }

    pub fn custom_submit_label(label: Option<&Value>) -> Option<String> {
        text(label, 40)
    }

    pub fn field(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.id == id)
    }

    /// A field's value now: the answer, else the default or empty.
    pub fn value(&self, id: &str) -> Value {
        match (self.values.get(id), self.field(id)) {
            (Some(value), _) => value.clone(),
            (None, Some(field)) => field.empty_value(),
            (None, None) => Value::Null,
        }
    }

    /// Every field's value checked and coerced: the answers to submit, or
    /// each failing field's id and message, in field order.
    pub fn validate(
        &self,
        values: &Map<String, Value>,
    ) -> Result<Map<String, Value>, Vec<(String, String)>> {
        self.validate_answers(values).map_err(|errors| {
            errors
                .into_iter()
                .map(|(id, error)| (id, error.render(crate::i18n::Language::English)))
                .collect()
        })
    }

    pub fn validate_answers(
        &self,
        values: &Map<String, Value>,
    ) -> Result<Map<String, Value>, Vec<(String, AnswerError)>> {
        let mut clean = Map::new();
        let mut errors = Vec::new();
        for field in &self.fields {
            let raw = values.get(&field.id).cloned().unwrap_or(Value::Null);
            match field.coerce_answer(&raw) {
                Ok(value) if field.required && Field::is_empty(&value) => {
                    let template = if field.kind == FieldType::Checkbox {
                        "{label} must be checked"
                    } else {
                        "{label} is required"
                    };
                    errors.push((
                        field.id.clone(),
                        field.answer_error(template, String::new()),
                    ));
                }
                Ok(value) => {
                    clean.insert(field.id.clone(), value);
                }
                Err(message) => errors.push((field.id.clone(), message)),
            }
        }
        if errors.is_empty() {
            Ok(clean)
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// The user submitted these answers (every field).
    Submit { values: Map<String, Value> },
}

impl Editable for Form {
    type Change = Change;

    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        if new.submitted && (!old.submitted || old.values != new.values) {
            vec![Change::Submit {
                values: new.values.clone(),
            }]
        } else {
            Vec::new()
        }
    }

    fn apply(&mut self, change: &Change) -> Result<(), String> {
        let Change::Submit { values } = change;
        let values = self.validate(values).map_err(|errors| {
            errors
                .into_iter()
                .map(|(_, message)| message)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        self.values = values;
        self.submitted = true;
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        let Change::Submit { values } = change;
        json!({"op": "submit", "values": values})
    }

    fn describe(change: &Change, _: &Self, after: &Self) -> String {
        let Change::Submit { values } = change;
        let answers: Vec<String> = after
            .fields
            .iter()
            .map(|f| {
                let value = values.get(&f.id).unwrap_or(&Value::Null);
                format!("{}: {}", f.label, truncate(&f.display(value), 120))
            })
            .collect();
        format!("submitted the form ({})", answers.join("; "))
    }

    fn to_value(&self) -> Value {
        let fields: Vec<Value> = self
            .fields
            .iter()
            .map(|f| {
                let mut map = Map::new();
                map.insert("id".into(), json!(f.id));
                map.insert("type".into(), json!(f.kind.name()));
                map.insert("label".into(), json!(f.label));
                if let Some(help) = &f.help {
                    map.insert("help".into(), json!(help));
                }
                if let Some(placeholder) = &f.placeholder {
                    map.insert("placeholder".into(), json!(placeholder));
                }
                if f.required {
                    map.insert("required".into(), json!(true));
                }
                if !f.options.is_empty() {
                    map.insert(
                        "options".into(),
                        f.options
                            .iter()
                            .map(|o| json!({"value": o.value, "label": o.label}))
                            .collect(),
                    );
                }
                for (key, bound) in [("min", f.min), ("max", f.max), ("step", f.step)] {
                    if let Some(bound) = bound {
                        map.insert(key.into(), json!(bound));
                    }
                }
                for (key, bound) in [("min", &f.min_date), ("max", &f.max_date)] {
                    if let Some(bound) = bound {
                        map.insert(key.into(), json!(bound));
                    }
                }
                if !f.default.is_null() {
                    map.insert("default".into(), f.default.clone());
                }
                Value::Object(map)
            })
            .collect();
        let mut map = Map::new();
        if let Some(description) = &self.description {
            map.insert("description".into(), json!(description));
        }
        map.insert("submitLabel".into(), json!(self.submit_label));
        map.insert("fields".into(), Value::Array(fields));
        map.insert("values".into(), Value::Object(self.values.clone()));
        map.insert("submitted".into(), json!(self.submitted));
        Value::Object(map)
    }
}

/// "3 fields" / "3 fields, 1 required".
pub fn field_count(form: &Form) -> String {
    field_count_in(form, crate::i18n::Language::English)
}

pub fn field_count_in(form: &Form, language: crate::i18n::Language) -> String {
    let required = form.fields.iter().filter(|f| f.required).count();
    let fields = super::edits::count_in(
        language,
        form.fields.len(),
        "{count} field",
        "{count} fields",
    );
    if required == 0 {
        fields
    } else {
        let required = super::edits::count_in(language, required, "1 required", "{count} required");
        format!("{fields}, {required}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn sample() -> Form {
        Form::parse(&json!({
            "description": "Deploy settings",
            "fields": [
                {"id": "name", "label": "Name", "required": true, "max": 10},
                {"id": "env", "type": "select", "label": "Environment", "options": ["staging", {"value": "prod", "label": "Production"}], "default": "staging"},
                {"id": "regions", "type": "multiselect", "options": ["eu", "us", "ap"], "max": 2},
                {"id": "replicas", "type": "number", "min": 1, "max": 9},
                {"id": "load", "type": "slider", "min": 0, "max": 100, "default": 50},
                {"id": "agree", "type": "checkbox", "label": "I agree", "required": true},
                {"id": "when", "type": "date", "min": "2026-01-01", "default": "nope"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn parses_fields_with_defaults() {
        let f = sample();
        assert_eq!(f.submit_label, "Submit");
        assert_eq!(f.value("env"), json!("staging"));
        assert_eq!(f.value("load"), json!(50.0));
        assert_eq!(f.value("agree"), json!(false));
        assert_eq!(f.value("regions"), json!([]));
        // An invalid default is dropped.
        assert_eq!(f.value("when"), Value::Null);
        assert_eq!(
            f.field("when").unwrap().min_date.as_deref(),
            Some("2026-01-01")
        );
        assert_eq!(f.field("env").unwrap().option_label("prod"), "Production");
        assert_eq!(field_count(&f), "7 fields, 2 required");
    }

    #[test]
    fn readable_errors() {
        let errors = Form::parse(&json!({"fields": [
            {"id": "a", "type": "color"},
            {"id": "b", "type": "radio"},
            {"label": "no id"},
            {"id": "c", "type": "number", "min": 5, "max": 1}
        ]}))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "field \"a\": unknown type \"color\" (one of text, textarea, number, select, multiselect, checkbox, radio, slider, date)",
                "field \"b\" (radio) needs options",
                "field #3 has no id",
                "field \"c\": min is greater than max",
            ]
        );
        assert_eq!(
            Form::parse(&json!({})).unwrap_err(),
            vec!["\"fields\" must be an array"]
        );
    }

    #[test]
    fn validation_reports_every_failing_field() {
        let f = sample();
        let values = json!({
            "name": "a very long name",
            "regions": ["eu", "us", "ap"],
            "replicas": "12",
            "when": "2025-12-31",
        });
        let errors = f.validate(values.as_object().unwrap()).unwrap_err();
        assert_eq!(
            errors,
            vec![
                ("name".into(), "Name takes at most 10 characters".into()),
                ("regions".into(), "regions: choose at most 2".into()),
                ("replicas".into(), "replicas must be at most 9".into()),
                ("agree".into(), "I agree must be checked".into()),
                ("when".into(), "when must be on or after 2026-01-01".into()),
            ]
        );
        assert!(!is_date("2026-02-30"));
        assert!(is_date("2028-02-29"));
    }

    #[test]
    fn submit_is_one_change_and_round_trips() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        let answers = json!({
            "name": " Ada ", "env": "prod", "regions": ["us", "eu"], "replicas": "3",
            "load": 75, "agree": true, "when": "2026-03-01",
        });
        let submit = Change::Submit {
            values: answers.as_object().unwrap().clone(),
        };
        edits.edit(&submit).unwrap();
        let form = edits.working();
        assert!(form.submitted);
        assert_eq!(form.values["name"], json!("Ada"));
        // Multiselect answers keep the options' order.
        assert_eq!(form.values["regions"], json!(["eu", "us"]));
        assert_eq!(form.values["replicas"], json!(3.0));
        let (values, lines) = edits.summarize();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["op"], "submit");
        assert_eq!(
            lines[0],
            "submitted the form (Name: Ada; Environment: Production; regions: eu, us; replicas: 3; load: 75; I agree: yes; when: 2026-03-01)"
        );
        assert_eq!(Form::parse(&form.to_value()).unwrap(), *form);
        // An agent update that reopens it (submitted false) keeps the values.
        let mut reopened = form.clone();
        reopened.submitted = false;
        assert!(Form::diff(&reopened, &reopened).is_empty());
        // A failing submit changes nothing.
        let bad = Change::Submit { values: Map::new() };
        assert_eq!(
            base.clone().apply(&bad).unwrap_err(),
            "Name is required; I agree must be checked"
        );
    }
}

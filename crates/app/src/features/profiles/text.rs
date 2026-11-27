//! App-owned profile copy is translated when rendered. Store the key and
//! arguments separately so pending confirmations, notices and errors can
//! follow a language change without repeating any reads or writes.

use gentle_core::i18n::{Language, tr};

#[derive(Clone, Debug)]
pub(super) enum ProfileText {
    Template(&'static str, Vec<(&'static str, Self)>),
    Raw(String),
    Selection(Option<gentle_core::profiles::RouteEntry>),
    Joined(Vec<Self>, &'static str),
}

impl From<String> for ProfileText {
    fn from(raw: String) -> Self {
        Self::Raw(raw)
    }
}

impl From<std::io::Error> for ProfileText {
    fn from(error: std::io::Error) -> Self {
        Self::Raw(error.to_string())
    }
}

impl From<&'static str> for ProfileText {
    fn from(key: &'static str) -> Self {
        Self::Template(key, Vec::new())
    }
}

impl ProfileText {
    pub(super) fn store_error((key, name): (&'static str, String)) -> Self {
        Self::Template(key, vec![("name", name.into())])
    }

    pub(super) fn render(&self) -> String {
        self.render_in(crate::shared::i18n::language())
    }

    pub(super) fn render_in(&self, language: Language) -> String {
        match self {
            Self::Raw(raw) => raw.clone(),
            Self::Selection(entry) => {
                gentle_core::profiles::RouteEntry::format_selection_in(entry.as_ref(), language)
            }
            Self::Joined(parts, separator) => parts
                .iter()
                .map(|part| part.render_in(language))
                .collect::<Vec<_>>()
                .join(tr(language, separator)),
            Self::Template(key, args) => {
                let mut tail = tr(language, key);
                let mut output = String::new();
                while let Some((prefix, rest)) = tail.split_once('{') {
                    output.push_str(prefix);
                    let Some((name, rest)) = rest.split_once('}') else {
                        output.push('{');
                        output.push_str(rest);
                        return output;
                    };
                    if let Some((_, value)) = args.iter().find(|(arg, _)| *arg == name) {
                        output.push_str(&value.render_in(language));
                    } else {
                        output.push('{');
                        output.push_str(name);
                        output.push('}');
                    }
                    tail = rest;
                }
                output.push_str(tail);
                output
            }
        }
    }
}

macro_rules! profile_msg {
    ($key:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::features::profiles::text::ProfileText::Template(
            $key,
            vec![$((stringify!($name), $value.into())),*],
        )
    };
}
pub(super) use profile_msg;

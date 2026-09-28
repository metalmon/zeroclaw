//! Minimal process-global localizer for config CRUD error messages.
//!
//! The runtime's Fluent i18n (`zeroclaw-runtime::i18n`) can't be reused here:
//! `zeroclaw-runtime` depends on `zeroclaw-config`, not the other way round, so
//! importing it would be a dependency cycle. This module is a small, self
//! contained Fluent loader for the `config-errors.ftl` catalogs, sharing the
//! same "process locale set once at startup, English fallback" model.
//!
//! The daemon calls [`init`] at startup with the configured locale (alongside
//! `zeroclaw_runtime::i18n::init`). Undefined keys — or any locale without a
//! translation — fall back to the embedded English catalog, then to `{key}`.

use fluent::{FluentArgs, FluentBundle, FluentResource};
use std::sync::OnceLock;

static LOCALE: OnceLock<String> = OnceLock::new();

const EN_FTL: &str = include_str!("../locales/en/config-errors.ftl");

fn builtin_ftl(locale: &str) -> Option<&'static str> {
    match locale {
        "ru" => Some(include_str!("../locales/ru/config-errors.ftl")),
        _ => None,
    }
}

/// Set the process locale for config error messages. Idempotent — only the
/// first call wins, matching `zeroclaw_runtime::i18n::init`.
pub fn init(locale: &str) {
    let _ = LOCALE.set(locale.to_string());
}

fn active_locale() -> &'static str {
    LOCALE.get().map(String::as_str).unwrap_or("en")
}

fn format_from(source: &str, locale: &str, key: &str, args: &[(&str, &str)]) -> Option<String> {
    let resource = FluentResource::try_new(source.to_string()).ok()?;
    let langid = locale.parse().unwrap_or_else(|_| "en".parse().unwrap());
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle.set_use_isolating(false);
    let _ = bundle.add_resource(resource);
    let msg = bundle.get_message(key)?;
    let pattern = msg.value()?;
    let mut fargs = FluentArgs::new();
    for (name, value) in args {
        fargs.set(*name, *value);
    }
    let mut errors = vec![];
    let out = bundle.format_pattern(pattern, Some(&fargs), &mut errors);
    if errors.is_empty() {
        Some(out.into_owned())
    } else {
        None
    }
}

/// Localize a config error message by key in the process locale, formatting the
/// given Fluent arguments. Falls back to the English catalog, then to `{key}`.
pub fn localize(key: &str, args: &[(&str, &str)]) -> String {
    let locale = active_locale();
    if locale != "en"
        && let Some(source) = builtin_ftl(locale)
        && let Some(value) = format_from(source, locale, key, args)
    {
        return value;
    }
    format_from(EN_FTL, "en", key, args).unwrap_or_else(|| format!("{{{key}}}"))
}

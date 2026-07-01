use std::env;

pub fn detect_lang_from_args(args: &[String]) -> Option<String> {
    let mut it = args.iter().skip(1);
    while let Some(arg) = it.next() {
        if arg == "--lang" {
            if let Some(v) = it.next() {
                if !v.trim().is_empty() {
                    return Some(v.to_string());
                }
            }
            continue;
        }
        if let Some(v) = arg.strip_prefix("--lang=") {
            if !v.trim().is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn locale_from_cli_or_env(cli_lang: Option<&str>) -> String {
    if let Some(lang) = cli_lang {
        return normalize_locale(lang);
    }

    if let Ok(lang) = env::var("TRACK_LANG") {
        if !lang.trim().is_empty() {
            return normalize_locale(&lang);
        }
    }

    if let Ok(lang) = env::var("LANG") {
        if !lang.trim().is_empty() {
            return normalize_locale(&lang);
        }
    }

    "en-US".to_string()
}

pub fn init_i18n(cli_lang: Option<&str>) -> String {
    let locale = locale_from_cli_or_env(cli_lang);
    rust_i18n::set_locale(&locale);
    locale
}

pub fn apply_clap_i18n(cmd: &mut clap::Command, root_key: &str) {
    let new_cmd = apply_clap_i18n_command(cmd.clone(), root_key);
    *cmd = new_cmd;
}

pub fn apply_help_i18n(cmd: &mut clap::Command, root_key: &str, locale: &str) {
    let new_cmd = apply_help_i18n_command(cmd.clone(), root_key, locale, true);
    *cmd = new_cmd;
}

fn normalize_locale(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return "en-US".to_string();
    }

    let s = s.replace('_', "-");
    let base = s.split('.').next().unwrap_or(&s);
    let base = base.split('@').next().unwrap_or(base);
    let mut parts = base.split('-');
    let lang = parts.next().unwrap_or("en").to_ascii_lowercase();
    let region = parts.next().unwrap_or("US").to_ascii_uppercase();
    format!("{}-{}", lang, region)
}

fn apply_clap_i18n_command(mut cmd: clap::Command, key_prefix: &str) -> clap::Command {
    if let Some(v) = lookup(&format!("{key_prefix}.about")) {
        cmd = cmd.about(v);
    }
    if let Some(v) = lookup(&format!("{key_prefix}.long_about")) {
        cmd = cmd.long_about(v);
    }
    if let Some(v) = lookup(&format!("{key_prefix}.after_help")) {
        cmd = cmd.after_help(v);
    }
    if let Some(v) = lookup(&format!("{key_prefix}.before_help")) {
        cmd = cmd.before_help(v);
    }

    let arg_ids: Vec<String> = cmd

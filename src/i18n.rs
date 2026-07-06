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

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
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    for arg_id in arg_ids {
        if let Some(v) = lookup(&format!("{key_prefix}.args.{arg_id}")) {
            cmd = cmd.mut_arg(arg_id.clone(), |a| a.help(v)).clone();
        }
        if let Some(v) = lookup(&format!("{key_prefix}.args_long.{arg_id}")) {
            cmd = cmd.mut_arg(arg_id.clone(), |a| a.long_help(v)).clone();
        }
    }

    let sub_names: Vec<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in sub_names {
        let child_key = format!("{key_prefix}.commands.{name}");
        cmd = cmd
            .mut_subcommand(name, |sub| apply_clap_i18n_command(sub, &child_key))
            .clone();
    }

    cmd
}

fn apply_help_i18n_command(
    mut cmd: clap::Command,
    root_key: &str,
    locale: &str,
    is_root: bool,
) -> clap::Command {
    let help_key = format!("{root_key}.help");
    let mut template = String::new();
    template.push_str("{about}\n\n");

    let usage_title = lookup(&format!("{help_key}.usage")).unwrap_or_else(|| "Usage".to_string());
    template.push_str(&format!("{usage_title}: {{usage}}\n\n"));

    let commands_title =
        lookup(&format!("{help_key}.commands")).unwrap_or_else(|| "Commands".to_string());
    template.push_str(&format!("{commands_title}:\n{{subcommands}}\n\n"));

    let options_title =
        lookup(&format!("{help_key}.options")).unwrap_or_else(|| "Options".to_string());
    template.push_str(&format!("{options_title}:\n{{options}}\n"));

    cmd = cmd.help_template(template).disable_help_subcommand(true);

    let is_zh = locale.to_ascii_lowercase().starts_with("zh");
    let help_short = if is_zh {
        "显示帮助信息"
    } else {
        "Print help"
    };
    let help_long = if is_zh {
        "显示帮助信息（使用 '-h' 查看摘要）"
    } else {
        "Print help (see a summary with '-h')"
    };
    let version_short = if is_zh {
        "显示版本信息"
    } else {
        "Print version"
    };

    cmd = cmd.disable_help_flag(true);

    let arg_ids: Vec<String> = cmd
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();

    if !arg_ids.iter().any(|id| id == "help") {
        cmd = cmd.arg(
            clap::Arg::new("help")
                .short('h')
                .long("help")
                .action(clap::ArgAction::Help)
                .global(true)
                .help(help_short)
                .long_help(help_long),
        );
    } else {
        cmd = cmd
            .mut_arg("help", |a| a.help(help_short).long_help(help_long))
            .clone();
    }

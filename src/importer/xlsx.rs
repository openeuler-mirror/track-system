use anyhow::{anyhow, Result};
use calamine::{open_workbook_auto, Data, Range, Reader};
use std::path::Path;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct XlsxPackageRow {
    pub name: String,
    pub level_ctyunos2: Option<i32>,
    pub level_ctyunos3: Option<i32>,
    pub level_ctyunos4: Option<i32>,
    pub branch_ctyunos2: Option<String>,
    pub branch_ctyunos3: Option<String>,
    pub branch_ctyunos4: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XlsxParseResult {
    pub sheet: String,
    pub rows: Vec<XlsxPackageRow>,
}

pub fn parse_xlsx_packages(path: impl AsRef<Path>) -> Result<XlsxParseResult> {
    let path = path.as_ref();
    debug!(file = %path.display(), "开始打开XLSX文件");
    let mut workbook = open_workbook_auto(path)?;
    let sheet_names = workbook.sheet_names().to_vec();
    debug!(sheets = ?sheet_names, "开始解析XLSX工作表");
    for sheet in sheet_names {
        let range = match workbook.worksheet_range(&sheet) {
            Ok(range) => range,
            Err(_) => continue,
        };
        debug!(sheet = %sheet, "开始检测表头");
        debug_preview_rows(&sheet, &range, 6, 12);
        if let Some((header_row_index, header_map)) = detect_header(&range) {
            debug!(sheet = %sheet, header_row_index, header_map = ?header_map, "检测到CTyunOS表头");
            let rows = parse_rows(&range, header_row_index, &header_map);
            debug!(sheet = %sheet, rows = rows.len(), "解析到行数据");
            return Ok(XlsxParseResult { sheet, rows });
        }
        if let Some(version) = parse_ctyunos_version_from_sheet_name(&sheet) {
            debug!(sheet = %sheet, version, "从工作表名称识别CTyunOS版本");
            if let Some((header_row_index, name_col, level_col, branch_col)) =
                detect_simple_header(&range)
            {
                let mut map = HeaderMap::default();
                map.name = Some(name_col);
                match version {
                    2 => {
                        map.level_ctyunos2 = Some(level_col);
                        map.branch_ctyunos2 = branch_col;
                    }
                    3 => {
                        map.level_ctyunos3 = Some(level_col);
                        map.branch_ctyunos3 = branch_col;
                    }
                    4 => {
                        map.level_ctyunos4 = Some(level_col);
                        map.branch_ctyunos4 = branch_col;
                    }
                    _ => {}
                }
                let rows = parse_rows(&range, header_row_index, &map);
                debug!(sheet = %sheet, header_row_index, header_map = ?map, rows = rows.len(), "使用简化表头解析");
                return Ok(XlsxParseResult { sheet, rows });
            }
        }
        debug!(sheet = %sheet, "未命中表头规则");
    }
    Err(anyhow!("未找到包含CTyunOS等级列的工作表"))
}

#[derive(Default, Debug)]
struct HeaderMap {
    name: Option<usize>,
    level_ctyunos2: Option<usize>,
    level_ctyunos3: Option<usize>,
    level_ctyunos4: Option<usize>,
    branch_ctyunos2: Option<usize>,
    branch_ctyunos3: Option<usize>,
    branch_ctyunos4: Option<usize>,
}

fn detect_header(range: &Range<Data>) -> Option<(usize, HeaderMap)> {
    let rows: Vec<&[Data]> = range.rows().collect();
    for row_index in 0..rows.len() {
        let row = rows[row_index];
        let prev_row = if row_index > 0 {
            rows.get(row_index - 1).copied()
        } else {
            None
        };
        let next_row = rows.get(row_index + 1).copied();
        let mut map = HeaderMap::default();
        let ctyunos_context = row_contains_ctyunos_context(row)
            || prev_row.map(row_contains_ctyunos_context).unwrap_or(false)
            || next_row.map(row_contains_ctyunos_context).unwrap_or(false);
        let version_row = if row_contains_version(row, ctyunos_context) {
            row
        } else if let Some(prev) = prev_row {
            if row_contains_version(prev, ctyunos_context) {
                prev
            } else {
                row
            }
        } else {
            row
        };
        let mut version_hint: Option<i32> = None;
        let mut version_hints = Vec::new();
        for cell in version_row.iter() {
            let raw = cell_to_string(cell);
            if let Some(version) = parse_ctyunos_version_from_raw(&raw, ctyunos_context) {
                version_hint = Some(version);
            }
            version_hints.push(version_hint);
        }
        let max_cols = next_row
            .map(|next| next.len().max(row.len()))
            .unwrap_or(row.len());
        let mut last_top_label = String::new();
        let mut last_bottom_label = String::new();
        for col_index in 0..max_cols {
            let raw_top = row.get(col_index).map(cell_to_string).unwrap_or_default();
            let raw_bottom = next_row
                .and_then(|next| next.get(col_index))
                .map(cell_to_string)
                .unwrap_or_default();
            if !raw_top.is_empty() {
                last_top_label = raw_top.clone();
            }
            if !raw_bottom.is_empty() {
                last_bottom_label = raw_bottom.clone();
            }
            let raw_top_effective = if raw_top.is_empty() {
                last_top_label.clone()
            } else {
                raw_top.clone()
            };
            let raw_bottom_effective = if raw_bottom.is_empty() {
                last_bottom_label.clone()
            } else {
                raw_bottom.clone()
            };
            if raw_top.is_empty() && raw_bottom.is_empty() {
                continue;
            }
            let normalized_top = normalize_header(&raw_top_effective);
            let normalized_bottom = normalize_header(&raw_bottom_effective);
            let normalized_combined =
                normalize_header(&format!("{}{}", raw_top_effective, raw_bottom_effective));
            if map.name.is_none()
                && (is_name_header(&normalized_top)
                    || is_name_header(&normalized_bottom)
                    || is_name_header(&normalized_combined))
            {
                map.name = Some(col_index);
                continue;
            }
            let is_branch = is_branch_header(&raw_top_effective, &raw_bottom_effective);
            let is_level = is_level_header(&raw_top_effective, &raw_bottom_effective);
            let detected_version =
                parse_ctyunos_version_from_raw(&raw_top_effective, ctyunos_context)
                    .or_else(|| {
                        parse_ctyunos_version_from_raw(&raw_bottom_effective, ctyunos_context)
                    })
                    .or_else(|| {
                        parse_ctyunos_version_from_raw(
                            &format!("{}{}", raw_top_effective, raw_bottom_effective),
                            ctyunos_context,
                        )
                    })
                    .or_else(|| parse_ctyunos_version(&normalized_top))
                    .or_else(|| parse_ctyunos_version(&normalized_bottom))
                    .or_else(|| parse_ctyunos_version(&normalized_combined))
                    .or_else(|| version_hints.get(col_index).copied().flatten());
            let is_ctyunos2 = detected_version == Some(2);
            let is_ctyunos3 = detected_version == Some(3);
            let is_ctyunos4 = detected_version == Some(4);
            if is_ctyunos2 {
                if is_branch {
                    map.branch_ctyunos2 = Some(col_index);
                } else if is_level {
                    map.level_ctyunos2 = Some(col_index);
                }
                continue;
            }
            if is_ctyunos3 {
                if is_branch {
                    map.branch_ctyunos3 = Some(col_index);
                } else if is_level {
                    map.level_ctyunos3 = Some(col_index);
                }
                continue;
            }
            if is_ctyunos4 {
                if is_branch {
                    map.branch_ctyunos4 = Some(col_index);
                } else if is_level {
                    map.level_ctyunos4 = Some(col_index);
                }
                continue;
            }
        }
        if map.name.is_some()
            && (map.level_ctyunos2.is_some()
                || map.level_ctyunos3.is_some()
                || map.level_ctyunos4.is_some())
        {
            debug!(
                header_row_index = row_index,
                header_map = ?map,
                "CTyunOS表头匹配成功"
            );
            return Some((row_index, map));
        }
    }
    None
}

fn parse_rows(
    range: &Range<Data>,
    header_row_index: usize,
    map: &HeaderMap,
) -> Vec<XlsxPackageRow> {
    let mut rows = Vec::new();
    for row in range.rows().skip(header_row_index + 1) {
        let name = map
            .name
            .and_then(|index| row.get(index))
            .map(cell_to_string)
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let level_ctyunos2 = map
            .level_ctyunos2
            .and_then(|index| row.get(index))
            .and_then(parse_level);
        let level_ctyunos3 = map
            .level_ctyunos3
            .and_then(|index| row.get(index))
            .and_then(parse_level);
        let level_ctyunos4 = map
            .level_ctyunos4
            .and_then(|index| row.get(index))
            .and_then(parse_level);
        let branch_ctyunos2 = map
            .branch_ctyunos2
            .and_then(|index| row.get(index))
            .map(cell_to_string)
            .and_then(normalize_branch);
        let branch_ctyunos3 = map
            .branch_ctyunos3
            .and_then(|index| row.get(index))
            .map(cell_to_string)
            .and_then(normalize_branch);
        let branch_ctyunos4 = map
            .branch_ctyunos4
            .and_then(|index| row.get(index))
            .map(cell_to_string)
            .and_then(normalize_branch);
        if level_ctyunos2.is_none() && level_ctyunos3.is_none() && level_ctyunos4.is_none() {
            continue;
        }
        rows.push(XlsxPackageRow {
            name,
            level_ctyunos2,
            level_ctyunos3,
            level_ctyunos4,
            branch_ctyunos2,
            branch_ctyunos3,
            branch_ctyunos4,
        });
    }
    rows
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{:.0}", value)
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn normalize_header(input: &str) -> String {
    input
        .chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '-' | '_' | '/' | '\\' | '(' | ')' | '（' | '）' | '.' | '：' | ':' | '·'
                )
        })
        .collect::<String>()
        .to_lowercase()
}

fn is_name_header(normalized: &str) -> bool {
    normalized.contains("软件包")
        || normalized.contains("包名")
        || normalized.contains("包名称")
        || normalized.contains("sourcepackage")
        || normalized.contains("sourcepkg")
        || normalized.contains("packagename")
        || normalized == "name"
}

fn is_ctyunos_version(normalized: &str, version: i32) -> bool {
    let key = format!("ctyunos{}", version);
    let key_v = format!("ctyunosv{}", version);
    let key_dot = format!("ctyunos{}0", version);
    let key_x = format!("ctyunos{}x", version);
    normalized.contains(&key)
        || normalized.contains(&key_v)
        || normalized.contains(&key_dot)
        || normalized.contains(&key_x)
}

fn parse_ctyunos_version(normalized: &str) -> Option<i32> {
    if is_ctyunos_version(normalized, 2) {
        return Some(2);
    }
    if is_ctyunos_version(normalized, 3) {
        return Some(3);
    }
    if is_ctyunos_version(normalized, 4) {
        return Some(4);
    }
    let normalized_with_alias = normalize_header(&normalized.replace("天翼云os", "ctyunos"));
    if normalized_with_alias != normalized {
        return parse_ctyunos_version(&normalized_with_alias);
    }
    None
}

fn parse_ctyunos_version_from_raw(raw: &str, ctyunos_context: bool) -> Option<i32> {
    let normalized = normalize_header(raw);
    if let Some(version) = parse_ctyunos_version(&normalized) {
        return Some(version);
    }
    if !ctyunos_context {
        return None;
    }
    let lower = raw.to_lowercase();
    if lower.contains("v2")
        || lower.contains("2.0")
        || lower.contains("2．0")
        || lower.contains("2版")
    {
        return Some(2);
    }
    if lower.contains("v3")
        || lower.contains("3.0")
        || lower.contains("3．0")
        || lower.contains("3版")
    {
        return Some(3);
    }
    if lower.contains("v4")
        || lower.contains("4.0")
        || lower.contains("4．0")
        || lower.contains("4版")
    {
        return Some(4);
    }
    if let Some(token) = extract_version_token(&lower) {
        if token.starts_with("22.") || token.starts_with("23.") {
            return Some(3);
        }
        if token.starts_with("24.") || token.starts_with("25.") {
            return Some(4);
        }
    }
    None
}

fn parse_ctyunos_version_from_sheet_name(sheet: &str) -> Option<i32> {
    parse_ctyunos_version_from_raw(sheet, true)
}

fn row_contains_ctyunos_context(row: &[Data]) -> bool {
    row.iter().any(|cell| {
        let raw = cell_to_string(cell);
        let lower = raw.to_lowercase();
        lower.contains("ctyunos")
            || raw.contains("天翼云os")
            || raw.contains("天翼云OS")
            || raw.contains("质量等级")
            || raw.contains("兼容性等级")
    })
}

fn row_contains_version(row: &[Data], ctyunos_context: bool) -> bool {
    row.iter().any(|cell| {
        parse_ctyunos_version_from_raw(&cell_to_string(cell), ctyunos_context).is_some()
    })
}

fn extract_version_token(input: &str) -> Option<String> {
    let mut token = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            token.push(ch);
        } else if !token.is_empty() {
            break;
        }
    }
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

fn detect_simple_header(range: &Range<Data>) -> Option<(usize, usize, usize, Option<usize>)> {
    for (row_index, row) in range.rows().enumerate() {
        let mut name_col = None;
        let mut level_col = None;
        let mut branch_col = None;
        for (col_index, cell) in row.iter().enumerate() {
            let raw = cell_to_string(cell);
            if raw.is_empty() {
                continue;
            }
            let normalized = normalize_header(&raw);
            if name_col.is_none() && is_name_header(&normalized) {
                name_col = Some(col_index);
                continue;
            }
            if level_col.is_none() && is_level_header(&raw, "") {
                level_col = Some(col_index);
                continue;
            }
            if branch_col.is_none() && is_branch_header(&raw, "") {
                branch_col = Some(col_index);
                continue;
            }
        }
        if let (Some(name_col), Some(level_col)) = (name_col, level_col) {
            return Some((row_index, name_col, level_col, branch_col));
        }
    }
    None
}

fn is_branch_header(raw_top: &str, raw_bottom: &str) -> bool {
    let lower_top = raw_top.to_lowercase();
    let lower_bottom = raw_bottom.to_lowercase();
    raw_top.contains("分支")
        || raw_bottom.contains("分支")
        || lower_top.contains("branch")
        || lower_bottom.contains("branch")
}

fn is_level_header(raw_top: &str, raw_bottom: &str) -> bool {
    let lower_top = raw_top.to_lowercase();
    let lower_bottom = raw_bottom.to_lowercase();
    raw_top.contains("等级")
        || raw_top.contains("重要性")
        || raw_top.contains("分级")
        || raw_top.contains("级别")
        || raw_top.contains("兼容性")
        || raw_bottom.contains("等级")
        || raw_bottom.contains("重要性")
        || raw_bottom.contains("分级")
        || raw_bottom.contains("级别")
        || raw_bottom.contains("兼容性")
        || lower_top.contains("level")
        || lower_top.contains("grade")
        || lower_top.contains("tier")
        || lower_bottom.contains("level")
        || lower_bottom.contains("grade")
        || lower_bottom.contains("tier")
}

fn parse_level(cell: &Data) -> Option<i32> {
    let raw = cell_to_string(cell);
    if raw.is_empty() {
        return None;
    }
    if let Ok(value) = raw.parse::<i32>() {
        return Some(value);
    }
    let lower = raw.to_lowercase();
    if let Some(digit) = lower.chars().find(|c| c.is_ascii_digit()) {
        return digit.to_string().parse::<i32>().ok();
    }
    if lower.contains('一') || lower.contains('Ⅰ') || lower.contains('i') {
        return Some(1);
    }
    if lower.contains('二') || lower.contains('Ⅱ') || lower.contains("ii") {
        return Some(2);
    }
    if lower.contains('三') || lower.contains('Ⅲ') || lower.contains("iii") {
        return Some(3);
    }
    None
}

fn normalize_branch(input: String) -> Option<String> {
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn debug_preview_rows(sheet: &str, range: &Range<Data>, row_limit: usize, col_limit: usize) {
    let mut previews = Vec::new();
    for (row_index, row) in range.rows().take(row_limit).enumerate() {
        let mut cols = Vec::new();
        for cell in row.iter().take(col_limit) {
            let value = cell_to_string(cell);
            if value.is_empty() {
                cols.push(String::from("∅"));
            } else {
                cols.push(value);
            }
        }
        previews.push((row_index, cols));
    }
    debug!(sheet = %sheet, previews = ?previews, "表头预览");
}

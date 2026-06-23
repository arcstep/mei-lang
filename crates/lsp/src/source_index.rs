use tower_lsp::lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

#[derive(Debug, Clone)]
pub struct IndexedSymbol {
    pub kind: &'static str,
    pub name: String,
    pub detail: String,
    pub selection_range: Range,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct IndexedReference {
    pub kind: &'static str,
    pub value: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct SourceIndex {
    pub symbols: Vec<IndexedSymbol>,
    pub references: Vec<IndexedReference>,
}

#[derive(Debug, Clone)]
struct LineInfo<'a> {
    index: usize,
    text: &'a str,
}

pub fn analyze_source(source: &str) -> SourceIndex {
    let lines = source
        .lines()
        .enumerate()
        .map(|(index, text)| LineInfo { index, text })
        .collect::<Vec<_>>();
    let mut symbols = Vec::new();
    let mut references = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = &lines[index];
        let trimmed = line.text.trim_start();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            index += 1;
            continue;
        }
        if let Some((kind, token)) = declaration_token(trimmed) {
            let (end_index, block) = collect_block(&lines, index);
            if let Some(symbol) = build_symbol(kind, token, index, end_index, &block) {
                symbols.push(symbol);
            }
            references.extend(scan_references(&block, index));
            index = end_index + 1;
            continue;
        }
        references.extend(scan_inline_references(line.text, line.index));
        index += 1;
    }
    SourceIndex {
        symbols,
        references,
    }
}

#[allow(deprecated)]
pub fn document_symbols(index: &SourceIndex) -> Vec<DocumentSymbol> {
    index
        .symbols
        .iter()
        .map(|symbol| DocumentSymbol {
            name: symbol.name.clone(),
            detail: Some(symbol.detail.clone()),
            kind: map_symbol_kind(symbol.kind),
            tags: None,
            deprecated: None,
            range: symbol.range,
            selection_range: symbol.selection_range,
            children: None,
        })
        .collect()
}

pub fn reference_at_position(index: &SourceIndex, position: Position) -> Option<IndexedReference> {
    index
        .references
        .iter()
        .find(|reference| range_contains(reference.range, position))
        .cloned()
}

pub fn symbol_at_position(index: &SourceIndex, position: Position) -> Option<IndexedSymbol> {
    index
        .symbols
        .iter()
        .find(|symbol| range_contains(symbol.selection_range, position))
        .cloned()
}

pub fn word_at_position(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let chars = line.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return None;
    }
    let mut column = position.character as usize;
    if column >= chars.len() {
        column = chars.len().saturating_sub(1);
    }
    if !is_word_char(chars[column]) {
        return None;
    }
    let mut start = column;
    let mut end = column;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    while end + 1 < chars.len() && is_word_char(chars[end + 1]) {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

fn declaration_token(trimmed: &str) -> Option<(&'static str, &'static str)> {
    [
        ("app", "app("),
        ("scene", "scene("),
        ("world", "world("),
        ("flow", "flow("),
        ("frame", "frame("),
        ("resource", "resource("),
        ("resource", "world.add_resource("),
        ("resource", "world_add_resource("),
        ("dataset", "world.add_dataset_view("),
        ("dataset", "world_add_dataset_view("),
        ("metric", "world.add_metric("),
        ("metric", "world_add_metric("),
        ("panel", "frame.add_panel("),
        ("panel", "panel("),
        ("component", "component("),
    ]
    .into_iter()
    .find(|(_, token)| trimmed.starts_with(*token))
}

fn collect_block(lines: &[LineInfo<'_>], start_index: usize) -> (usize, String) {
    let mut balance = 0i64;
    let mut started = false;
    let mut end_index = start_index;
    let mut block = String::new();
    for line in &lines[start_index..] {
        if !block.is_empty() {
            block.push('\n');
        }
        block.push_str(line.text);
        balance += paren_delta(line.text);
        if line.text.contains('(') {
            started = true;
        }
        end_index = line.index;
        if started && balance <= 0 {
            break;
        }
    }
    (end_index, block)
}

fn paren_delta(text: &str) -> i64 {
    let mut delta = 0i64;
    let mut in_string = false;
    let mut quote = '\0';
    let mut escaped = false;
    for ch in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_string = true;
                quote = ch;
            }
            '(' => delta += 1,
            ')' => delta -= 1,
            '#' => break,
            _ => {}
        }
    }
    delta
}

fn build_symbol(
    kind: &'static str,
    token: &'static str,
    start_line: usize,
    end_line: usize,
    block: &str,
) -> Option<IndexedSymbol> {
    let selection_range = if kind == "component" {
        find_first_string_arg(block, token, start_line)
    } else {
        find_named_string_arg(block, "id", start_line)
            .or_else(|| find_named_string_arg(block, "scene_id", start_line))
            .or_else(|| find_named_string_arg(block, "title", start_line))
    };
    let (name, selection_range) = match selection_range {
        Some((name, range)) => (name, range),
        None => (
            kind.to_string(),
            Range::new(
                Position::new(start_line as u32, 0),
                Position::new(start_line as u32, token.len() as u32),
            ),
        ),
    };
    Some(IndexedSymbol {
        kind,
        detail: kind.to_string(),
        name,
        selection_range,
        range: Range::new(
            Position::new(start_line as u32, 0),
            Position::new(
                end_line as u32,
                block.lines().last().map(|line| line.len()).unwrap_or(0) as u32,
            ),
        ),
    })
}

fn scan_references(block: &str, start_line: usize) -> Vec<IndexedReference> {
    let mut refs = Vec::new();
    for (offset, line) in block.lines().enumerate() {
        refs.extend(scan_inline_references(line, start_line + offset));
    }
    refs
}

fn scan_inline_references(line: &str, line_index: usize) -> Vec<IndexedReference> {
    let mut refs = Vec::new();
    for (kind, token) in [
        ("scene_file", "scene_file_ref("),
        ("scene", "scene_ref("),
        ("world", "world_ref("),
        ("frame", "frame_ref("),
        ("resource", "resource_ref("),
        ("dataset", "dataset_ref("),
        ("metric", "metric_ref("),
        ("component", "component("),
    ] {
        if let Some(reference) = find_call_reference(line, line_index, kind, token) {
            refs.push(reference);
        }
    }
    if let Some(reference) = find_named_reference(line, line_index, "scene_file", "scene_file") {
        refs.push(reference);
    }
    refs
}

fn find_call_reference(
    line: &str,
    line_index: usize,
    kind: &'static str,
    token: &'static str,
) -> Option<IndexedReference> {
    let start = line.find(token)?;
    let quote = line[start + token.len()..].find('"')? + start + token.len();
    let rest = &line[quote + 1..];
    let end_quote = rest.find('"')? + quote + 1;
    Some(IndexedReference {
        kind,
        value: line[quote + 1..end_quote].to_string(),
        range: Range::new(
            Position::new(line_index as u32, (quote + 1) as u32),
            Position::new(line_index as u32, end_quote as u32),
        ),
    })
}

fn find_named_reference(
    line: &str,
    line_index: usize,
    kind: &'static str,
    key: &'static str,
) -> Option<IndexedReference> {
    let needle = format!("{key} = ");
    let start = line.find(&needle)?;
    let quote = line[start + needle.len()..].find('"')? + start + needle.len();
    let rest = &line[quote + 1..];
    let end_quote = rest.find('"')? + quote + 1;
    Some(IndexedReference {
        kind,
        value: line[quote + 1..end_quote].to_string(),
        range: Range::new(
            Position::new(line_index as u32, (quote + 1) as u32),
            Position::new(line_index as u32, end_quote as u32),
        ),
    })
}

fn find_named_string_arg(block: &str, key: &str, start_line: usize) -> Option<(String, Range)> {
    let needle = format!("{key} = ");
    for (offset, line) in block.lines().enumerate() {
        let Some(pos) = line.find(&needle) else {
            continue;
        };
        let Some(quote_offset) = line[pos + needle.len()..].find('"') else {
            continue;
        };
        let quote = quote_offset + pos + needle.len();
        let rest = &line[quote + 1..];
        let Some(end_quote_offset) = rest.find('"') else {
            continue;
        };
        let end_quote = end_quote_offset + quote + 1;
        return Some((
            line[quote + 1..end_quote].to_string(),
            Range::new(
                Position::new((start_line + offset) as u32, (quote + 1) as u32),
                Position::new((start_line + offset) as u32, end_quote as u32),
            ),
        ));
    }
    None
}

fn find_first_string_arg(block: &str, token: &str, start_line: usize) -> Option<(String, Range)> {
    for (offset, line) in block.lines().enumerate() {
        let Some(pos) = line.find(token) else {
            continue;
        };
        let Some(quote_offset) = line[pos + token.len()..].find('"') else {
            continue;
        };
        let quote = quote_offset + pos + token.len();
        let rest = &line[quote + 1..];
        let Some(end_quote_offset) = rest.find('"') else {
            continue;
        };
        let end_quote = end_quote_offset + quote + 1;
        return Some((
            line[quote + 1..end_quote].to_string(),
            Range::new(
                Position::new((start_line + offset) as u32, (quote + 1) as u32),
                Position::new((start_line + offset) as u32, end_quote as u32),
            ),
        ));
    }
    None
}

fn map_symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "app" => SymbolKind::OBJECT,
        "scene" => SymbolKind::MODULE,
        "world" => SymbolKind::NAMESPACE,
        "flow" => SymbolKind::EVENT,
        "frame" => SymbolKind::STRUCT,
        "panel" => SymbolKind::FIELD,
        "resource" => SymbolKind::VARIABLE,
        "dataset" => SymbolKind::ARRAY,
        "metric" => SymbolKind::PROPERTY,
        "component" => SymbolKind::CLASS,
        _ => SymbolKind::OBJECT,
    }
}

fn range_contains(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

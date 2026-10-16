use crate::markup::{Block, Cell, Inline, List, Table};

pub fn write(blocks: &[Block]) -> String {
    let mut out = String::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n");
        }
        write_block(block, &mut out);
    }
    out.trim_end().to_string()
}

fn write_block(block: &Block, out: &mut String) {
    match block {
        Block::Paragraph(inlines) => write_inlines(inlines, out),
        Block::Heading { level, content } => {
            let level = (*level).clamp(1, 6);
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
            write_inlines(content, out);
        }
        Block::List(list) => write_list(list, 0, out),
        Block::Pre { language, text } => {
            let fence = fence_for(text, 3);
            out.push_str(&fence);
            if let Some(language) = language {
                out.push_str(language);
            }
            out.push('\n');
            out.push_str(text);
            out.push('\n');
            out.push_str(&fence);
        }
        Block::Quote(blocks) => {
            let inner = write(blocks);
            for (index, line) in inner.lines().enumerate() {
                if index > 0 {
                    out.push('\n');
                }
                out.push_str("> ");
                out.push_str(line);
            }
        }
        Block::Rule => out.push_str("---"),
        Block::Table(table) => write_table(table, out),
        Block::Raw(html) => out.push_str(html),
    }
}

// A run of backticks longer than any in the content, so the fence cannot be
// closed early by the code itself. An inline span needs one backtick, a block
// fence needs three.
fn fence_for(text: &str, minimum: usize) -> String {
    let longest = text.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(minimum - 1) + 1)
}

fn write_list(list: &List, depth: usize, out: &mut String) {
    for (index, item) in list.items.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let marker = if list.ordered { "1." } else { "-" };
        let indent = "  ".repeat(depth);
        let inner = item_text(item);
        for (line_index, line) in inner.lines().enumerate() {
            if line_index > 0 {
                out.push('\n');
                out.push_str(&indent);
                out.push_str("  ");
                out.push_str(line);
                continue;
            }
            out.push_str(&indent);
            out.push_str(marker);
            out.push(' ');
            out.push_str(line);
        }
    }
}

fn item_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        match block {
            Block::List(nested) => {
                let mut nested_out = String::new();
                write_list(nested, 1, &mut nested_out);
                out.push_str(&nested_out);
            }
            other => write_block(other, &mut out),
        }
    }
    out
}

fn write_table(table: &Table, out: &mut String) {
    let columns = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(0);
    if columns == 0 {
        return;
    }

    let mut header: Option<&crate::markup::Row> = None;
    let mut body: Vec<&crate::markup::Row> = Vec::new();
    for (index, row) in table.rows.iter().enumerate() {
        if index == 0 && row.cells.iter().all(|cell| cell.header) {
            header = Some(row);
        } else {
            body.push(row);
        }
    }

    out.push('|');
    match header {
        Some(row) => {
            write_row(row, columns, out);
            out.push('\n');
            out.push('|');
            for _ in 0..columns {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
        None => {
            for index in 0..columns {
                if index > 0 {
                    out.push(' ');
                }
                out.push_str("  |");
            }
            out.push('\n');
            out.push('|');
            for _ in 0..columns {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    for (index, row) in body.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push('|');
        write_row(row, columns, out);
    }
    while out.ends_with('\n') {
        out.pop();
    }
}

fn write_row(row: &crate::markup::Row, columns: usize, out: &mut String) {
    for index in 0..columns {
        out.push(' ');
        if let Some(cell) = row.cells.get(index) {
            let mut text = String::new();
            write_cell(cell, &mut text);
            out.push_str(&text.replace('\n', " ").replace('|', "\\|"));
        }
        out.push_str(" |");
    }
}

fn write_cell(cell: &Cell, out: &mut String) {
    for (index, block) in cell.content.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        match block {
            Block::Paragraph(inlines) => write_inlines(inlines, out),
            Block::Raw(html) => out.push_str(html),
            other => write_block(other, out),
        }
    }
}

pub fn write_inlines(inlines: &[Inline], out: &mut String) {
    for (index, inline) in inlines.iter().enumerate() {
        write_inline(inline, index, inlines, out);
    }
}

fn write_inline(inline: &Inline, index: usize, siblings: &[Inline], out: &mut String) {
    match inline {
        Inline::Text(text) => out.push_str(&escape_text(text, index == 0)),
        Inline::Bold(content) => wrap(content, "**", out),
        Inline::Italic(content) => wrap(content, "*", out),
        Inline::Struck(content) => wrap(content, "~~", out),
        Inline::Under(content) => html_wrap("u", content, out),
        Inline::Sub(content) => html_wrap("sub", content, out),
        Inline::Sup(content) => html_wrap("sup", content, out),
        Inline::Code(text) => {
            let ticks = fence_for(text, 1);
            out.push_str(&ticks);
            out.push_str(text);
            out.push_str(&ticks);
        }
        Inline::Link { target, label } => {
            let _ = siblings;
            out.push('[');
            write_inlines(label, out);
            out.push_str("](");
            out.push_str(&encode_target(target));
            out.push(')');
        }
        Inline::External { url, label } => {
            out.push('[');
            write_inlines(label, out);
            out.push_str("](");
            out.push_str(url);
            out.push(')');
        }
        Inline::Image { target, alt } => {
            out.push_str("![");
            write_inlines(alt, out);
            out.push_str("](");
            out.push_str(&encode_target(target));
            out.push(')');
        }
        Inline::LineBreak => out.push_str("  \n"),
        Inline::Raw(markup) => out.push_str(markup),
    }
}

fn wrap(content: &[Inline], marks: &str, out: &mut String) {
    out.push_str(marks);
    write_inlines(content, out);
    out.push_str(marks);
}

fn html_wrap(tag: &str, content: &[Inline], out: &mut String) {
    out.push('<');
    out.push_str(tag);
    out.push('>');
    write_inlines(content, out);
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

// Markdown's own metacharacters, so text that merely looks like markup stays
// text. Braces and pipes need no escape here, unlike wikitext.
fn escape_text(text: &str, at_line_start: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, ch) in text.chars().enumerate() {
        let escape = match ch {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' => true,
            // A leading `#`, `-` or `+` would start a heading or a list.
            '#' | '-' | '+' => at_line_start && index == 0,
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn encode_target(target: &str) -> String {
    let mut out = String::with_capacity(target.len());
    for ch in target.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '(' | ')' => {
                out.push('%');
                out.push_str(&format!("{:02X}", ch as u32));
            }
            '\n' | '\r' | '\t' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

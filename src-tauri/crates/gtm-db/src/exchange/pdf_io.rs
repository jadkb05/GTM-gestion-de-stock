use super::{Report, error::ExchangeError, generated_at};
use std::fs;
use std::path::Path;

const PAGE_WIDTH: f32 = 842.0;
const PAGE_HEIGHT: f32 = 595.0;
const MARGIN: f32 = 36.0;

pub(super) fn write_pdf(path: &Path, report: &Report) -> Result<(), ExchangeError> {
    let pages = layout(report);
    fs::write(path, render(&pages))?;
    Ok(())
}

struct Page {
    content: String,
}

fn layout(report: &Report) -> Vec<Page> {
    let columns = report.headers.len().max(1);
    let width = (PAGE_WIDTH - MARGIN * 2.0) / columns as f32;
    let mut pages = Vec::new();
    let mut commands = String::new();
    let mut y = PAGE_HEIGHT - MARGIN;
    y = draw_title(&mut commands, y, report);
    y = draw_headers(&mut commands, y, &report.headers, width);
    if report.rows.is_empty() {
        y -= 16.0;
        text(&mut commands, "F1", 9.0, MARGIN, y, "Aucune ligne.");
    }
    let mut page_rows = 0;
    for row in &report.rows {
        if y < MARGIN + 28.0 {
            pages.push(Page {
                content: std::mem::take(&mut commands),
            });
            y = PAGE_HEIGHT - MARGIN;
            y = draw_headers(&mut commands, y, &report.headers, width);
        }
        y -= 14.0;
        for (index, cell) in row.iter().enumerate() {
            let fitted = fit(cell, (width / 4.4) as usize);
            text(
                &mut commands,
                "F1",
                8.0,
                MARGIN + width * index as f32,
                y,
                &fitted,
            );
        }
        page_rows += 1;
        let _ = page_rows;
    }
    pages.push(Page { content: commands });
    pages
}

fn draw_title(commands: &mut String, mut y: f32, report: &Report) -> f32 {
    text(commands, "F2", 14.0, MARGIN, y, &report.title);
    y -= 16.0;
    text(
        commands,
        "F1",
        9.0,
        MARGIN,
        y,
        &format!("Généré le {}", generated_at()),
    );
    y -= 12.0;
    text(commands, "F1", 9.0, MARGIN, y, &fit(&report.scope, 160));
    y -= 8.0;
    y
}

fn draw_headers(commands: &mut String, mut y: f32, headers: &[String], width: f32) -> f32 {
    y -= 16.0;
    for (index, header) in headers.iter().enumerate() {
        text(
            commands,
            "F2",
            8.0,
            MARGIN + width * index as f32,
            y,
            &fit(header, (width / 4.6) as usize),
        );
    }
    y -= 4.0;
    y
}

fn text(commands: &mut String, font: &str, size: f32, x: f32, y: f32, value: &str) {
    let literal = pdf_literal(&winansi(value));
    commands.push_str(&format!(
        "BT /{font} {size} Tf {x:.1} {y:.1} Td {literal} Tj ET\n"
    ));
}

fn fit(value: &str, max_chars: usize) -> String {
    let flat = value.replace(['\n', '\r'], " ");
    let max_chars = max_chars.max(4);
    let count = flat.chars().count();
    if count <= max_chars {
        return flat;
    }
    let mut kept = flat.chars().take(max_chars - 1).collect::<String>();
    kept.push('…');
    kept
}

fn winansi(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|char| match char {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            // Signes sans glyphe WinAnsi, remplacés par leur équivalent simple.
            '−' | '‐' | '‑' | '‒' => b'-',
            '\u{2009}' | '\u{202F}' | '\u{2007}' => b' ',
            // Les caractères de contrôle C1 n'ont pas de glyphe.
            other if (0x80..=0x9F).contains(&(other as u32)) => b'?',
            other if (other as u32) <= 0xFF => other as u8,
            _ => b'?',
        })
        .collect()
}

fn pdf_literal(bytes: &[u8]) -> String {
    let mut literal = String::from("(");
    for &byte in bytes {
        match byte {
            b'\\' | b'(' | b')' => {
                literal.push('\\');
                literal.push(byte as char);
            }
            32..=126 => literal.push(byte as char),
            _ => literal.push_str(&format!("\\{byte:03o}")),
        }
    }
    literal.push(')');
    literal
}

fn render(pages: &[Page]) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let font_regular = add(
        &mut objects,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    let font_bold = add(
        &mut objects,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    let mut page_ids = Vec::new();
    let mut content_ids = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let mut stream = page.content.clone();
        stream.push_str(&format!(
            "BT /F1 8 Tf {MARGIN:.1} 22 Td (Page {}) Tj ET\n",
            index + 1
        ));
        let bytes = stream.into_bytes();
        let mut body = format!("<< /Length {} >>\nstream\n", bytes.len()).into_bytes();
        body.extend_from_slice(&bytes);
        body.extend_from_slice(b"\nendstream");
        content_ids.push(add(&mut objects, body));
    }
    let pages_id_placeholder = objects.len() + pages.len() + 1;
    for content_id in &content_ids {
        let body = format!(
            "<< /Type /Page /Parent {pages_id_placeholder} 0 R /MediaBox [0 0 {PAGE_WIDTH:.0} {PAGE_HEIGHT:.0}] /Resources << /Font << /F1 {font_regular} 0 R /F2 {font_bold} 0 R >> >> /Contents {content_id} 0 R >>"
        );
        page_ids.push(add(&mut objects, body.into_bytes()));
    }
    let kids = page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    let pages_id = add(
        &mut objects,
        format!(
            "<< /Type /Pages /Count {} /Kids [{kids}] >>",
            page_ids.len()
        )
        .into_bytes(),
    );
    debug_assert_eq!(pages_id, pages_id_placeholder);
    let catalog_id = add(
        &mut objects,
        format!("<< /Type /Catalog /Pages {pages_id} 0 R >>").into_bytes(),
    );
    let mut output = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(output.len());
        output.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        output.extend_from_slice(object);
        output.extend_from_slice(b"\nendobj\n");
    }
    let xref = output.len();
    output.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    output.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    output.extend_from_slice(
        format!(
            "trailer << /Size {} /Root {catalog_id} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    output
}

fn add(objects: &mut Vec<Vec<u8>>, body: Vec<u8>) -> usize {
    objects.push(body);
    objects.len()
}

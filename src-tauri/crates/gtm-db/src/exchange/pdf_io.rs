use super::{Report, error::ExchangeError, generated_at};
use std::fs;
use std::path::Path;

const PAGE_WIDTH: f32 = 842.0;
const PAGE_HEIGHT: f32 = 595.0;
const MARGIN: f32 = 36.0;
const BODY_SIZE: f32 = 8.0;
const LINE_HEIGHT: f32 = 10.0;
/// Espace laissé de chaque côté du texte dans une colonne.
const CELL_PADDING: f32 = 3.0;
/// Pied de page : le numéro de page est écrit sous cette limite.
const BOTTOM: f32 = MARGIN;

pub(super) fn write_pdf(path: &Path, report: &Report) -> Result<(), ExchangeError> {
    let pages = layout(report);
    fs::write(path, render(&pages))?;
    Ok(())
}

struct Page {
    content: String,
}

#[derive(Clone, Copy)]
enum Font {
    Regular,
    Bold,
}

impl Font {
    fn name(self) -> &'static str {
        match self {
            Font::Regular => "F1",
            Font::Bold => "F2",
        }
    }
}

/// Mise en page : chaque cellule est écrite en entier. Les colonnes prennent
/// la largeur de leur contenu quand elle tient ; sinon le texte passe à la
/// ligne dans la colonne, et une rangée trop haute continue page suivante.
fn layout(report: &Report) -> Vec<Page> {
    let widths = column_widths(&report.headers, &report.rows);
    let mut writer = PageWriter::new(report, widths);
    writer.title();
    writer.headers();
    if report.rows.is_empty() {
        writer.y -= 16.0;
        text(
            &mut writer.commands,
            Font::Regular,
            9.0,
            MARGIN,
            writer.y,
            "Aucune ligne.",
        );
    }
    for row in &report.rows {
        writer.row(row);
    }
    writer.finish()
}

struct PageWriter<'a> {
    report: &'a Report,
    widths: Vec<f32>,
    pages: Vec<Page>,
    commands: String,
    y: f32,
}

impl<'a> PageWriter<'a> {
    fn new(report: &'a Report, widths: Vec<f32>) -> Self {
        Self {
            report,
            widths,
            pages: Vec::new(),
            commands: String::new(),
            y: PAGE_HEIGHT - MARGIN,
        }
    }

    fn title(&mut self) {
        let width = PAGE_WIDTH - MARGIN * 2.0;
        for line in wrap(&self.report.title, Font::Bold, 14.0, width) {
            text(&mut self.commands, Font::Bold, 14.0, MARGIN, self.y, &line);
            self.y -= 16.0;
        }
        let generated = format!("Généré le {}", generated_at());
        text(
            &mut self.commands,
            Font::Regular,
            9.0,
            MARGIN,
            self.y,
            &generated,
        );
        for line in wrap(&self.report.scope, Font::Regular, 9.0, width) {
            self.y -= 12.0;
            text(
                &mut self.commands,
                Font::Regular,
                9.0,
                MARGIN,
                self.y,
                &line,
            );
        }
        self.y -= 8.0;
    }

    fn headers(&mut self) {
        let cells = self
            .report
            .headers
            .iter()
            .zip(&self.widths)
            .map(|(header, width)| wrap(header, Font::Bold, BODY_SIZE, inner(*width)))
            .collect::<Vec<_>>();
        let lines = cells.iter().map(Vec::len).max().unwrap_or(1);
        for index in 0..lines {
            self.y -= LINE_HEIGHT;
            self.draw_line(&cells, index, Font::Bold);
        }
        self.y -= 4.0;
        rule(&mut self.commands, self.y, 0.6);
    }

    fn row(&mut self, row: &[String]) {
        let cells = self
            .widths
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let value = row.get(index).map(String::as_str).unwrap_or("");
                wrap(value, Font::Regular, BODY_SIZE, inner(*width))
            })
            .collect::<Vec<_>>();
        let lines = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let height = lines as f32 * LINE_HEIGHT + 4.0;
        // Une rangée qui tient sur une page n'est pas coupée.
        if self.y - height < BOTTOM && height <= self.capacity() {
            self.new_page();
        }
        for index in 0..lines {
            if self.y - LINE_HEIGHT < BOTTOM {
                self.new_page();
            }
            self.y -= LINE_HEIGHT;
            self.draw_line(&cells, index, Font::Regular);
        }
        self.y -= 4.0;
        rule(&mut self.commands, self.y, 0.85);
    }

    fn draw_line(&mut self, cells: &[Vec<String>], index: usize, font: Font) {
        let mut x = MARGIN;
        for (lines, width) in cells.iter().zip(&self.widths) {
            if let Some(line) = lines.get(index).filter(|line| !line.is_empty()) {
                text(
                    &mut self.commands,
                    font,
                    BODY_SIZE,
                    x + CELL_PADDING,
                    self.y + 2.0,
                    line,
                );
            }
            x += width;
        }
    }

    /// Hauteur disponible pour les rangées sur une page neuve.
    fn capacity(&self) -> f32 {
        let header_lines = self
            .report
            .headers
            .iter()
            .zip(&self.widths)
            .map(|(header, width)| wrap(header, Font::Bold, BODY_SIZE, inner(*width)).len())
            .max()
            .unwrap_or(1);
        PAGE_HEIGHT - MARGIN - BOTTOM - header_lines as f32 * LINE_HEIGHT - 4.0
    }

    fn new_page(&mut self) {
        self.pages.push(Page {
            content: std::mem::take(&mut self.commands),
        });
        self.y = PAGE_HEIGHT - MARGIN;
        self.headers();
    }

    fn finish(mut self) -> Vec<Page> {
        self.pages.push(Page {
            content: self.commands,
        });
        self.pages
    }
}

fn inner(width: f32) -> f32 {
    (width - CELL_PADDING * 2.0).max(1.0)
}

/// Largeur des colonnes : une colonne dont le contenu tient dans sa part
/// garde sa largeur naturelle ; l'espace restant est partagé entre les autres.
fn column_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<f32> {
    let available = PAGE_WIDTH - MARGIN * 2.0;
    let count = headers.len().max(1);
    let natural = (0..count)
        .map(|column| {
            let header = headers
                .get(column)
                .map(|value| longest_line(value, Font::Bold))
                .unwrap_or(0.0);
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|value| longest_line(value, Font::Regular))
                .fold(header, f32::max)
                + CELL_PADDING * 2.0
        })
        .collect::<Vec<_>>();
    let mut widths = vec![None; count];
    loop {
        let fixed: f32 = widths.iter().flatten().sum();
        let open = widths.iter().filter(|width| width.is_none()).count();
        if open == 0 {
            break;
        }
        let share = (available - fixed) / open as f32;
        let mut changed = false;
        for (column, width) in widths.iter_mut().enumerate() {
            if width.is_none() && natural[column] <= share {
                *width = Some(natural[column]);
                changed = true;
            }
        }
        if !changed {
            for width in widths.iter_mut().filter(|width| width.is_none()) {
                *width = Some(share);
            }
        }
    }
    let widths = widths.into_iter().flatten().collect::<Vec<f32>>();
    // L'espace libre est réparti pour aligner le tableau sur la largeur utile.
    let spare = (available - widths.iter().sum::<f32>()) / count as f32;
    widths
        .into_iter()
        .map(|width| width + spare.max(0.0))
        .collect()
}

fn longest_line(value: &str, font: Font) -> f32 {
    value
        .lines()
        .map(|line| text_width(line, font, BODY_SIZE))
        .fold(0.0, f32::max)
}

/// Découpe le texte en lignes d'au plus `width` points : aux espaces d'abord,
/// à l'intérieur d'un mot seulement s'il est plus large que la colonne.
fn wrap(value: &str, font: Font, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in value.split(['\n', '\r']).filter(|part| !part.is_empty()) {
        let mut line = String::new();
        for word in paragraph.split([' ', '\t']).filter(|word| !word.is_empty()) {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if text_width(&candidate, font, size) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for char in word.chars() {
                let mut next = line.clone();
                next.push(char);
                if !line.is_empty() && text_width(&next, font, size) > width {
                    lines.push(std::mem::replace(&mut line, char.to_string()));
                } else {
                    line = next;
                }
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Largeur en points du texte tel qu'il sera écrit (codage WinAnsi).
fn text_width(value: &str, font: Font, size: f32) -> f32 {
    let table = match font {
        Font::Regular => &REGULAR_WIDTHS,
        Font::Bold => &BOLD_WIDTHS,
    };
    let units: u32 = winansi(value)
        .into_iter()
        .map(|byte| {
            byte.checked_sub(32)
                .map(|index| u32::from(table[usize::from(index)]))
                .unwrap_or(0)
        })
        .sum();
    units as f32 * size / 1000.0
}

fn rule(commands: &mut String, y: f32, gray: f32) {
    commands.push_str(&format!(
        "{gray:.2} G 0.5 w {MARGIN:.1} {y:.1} m {:.1} {y:.1} l S 0 G\n",
        PAGE_WIDTH - MARGIN
    ));
}

fn text(commands: &mut String, font: Font, size: f32, x: f32, y: f32, value: &str) {
    let literal = pdf_literal(&winansi(value));
    let font = font.name();
    commands.push_str(&format!(
        "BT /{font} {size} Tf {x:.1} {y:.1} Td {literal} Tj ET\n"
    ));
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

/// Helvetica : chasses Adobe (AFM, millièmes d'em) des codes WinAnsi 32 à 255.
/// Codes sans glyphe : 0x7f, 0x81, 0x8d, 0x8f, 0x90, 0x9d.
const REGULAR_WIDTHS: [u16; 224] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
    0, 556, 0, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 222, 222,
    333, 333, 350, 556, 1000, 333, 1000, 500, 333, 944, 0, 500, 667, 278, 333, 556, 556, 556, 556,
    260, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 556, 537, 278, 333,
    333, 365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667, 1000, 722, 667, 667, 667, 667,
    278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667,
    611, 556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278, 278, 556, 556,
    556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
];
/// Helvetica-Bold : chasses Adobe (AFM, millièmes d'em) des codes WinAnsi 32 à 255.
/// Codes sans glyphe : 0x7f, 0x81, 0x8d, 0x8f, 0x90, 0x9d.
const BOLD_WIDTHS: [u16; 224] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
    0, 556, 0, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 278, 278,
    500, 500, 350, 556, 1000, 333, 1000, 556, 333, 944, 0, 500, 667, 278, 333, 556, 556, 556, 556,
    280, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 611, 556, 278, 333,
    333, 365, 556, 834, 834, 834, 611, 722, 722, 722, 722, 722, 722, 1000, 722, 667, 667, 667, 667,
    278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667,
    611, 556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556, 278, 278, 278, 278, 611, 611,
    611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611, 556,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Texte écrit par le PDF : police, taille, position et octets WinAnsi.
    struct Drawn {
        page: usize,
        font: String,
        size: f32,
        x: f32,
        bytes: Vec<u8>,
    }

    fn drawn(pages: &[Page]) -> Vec<Drawn> {
        let mut out = Vec::new();
        for (page, content) in pages.iter().enumerate() {
            for line in content
                .content
                .lines()
                .filter(|line| line.starts_with("BT /"))
            {
                let mut parts = line[4..].splitn(5, ' ');
                let font = parts.next().unwrap().to_string();
                let size = parts.next().unwrap().parse().unwrap();
                parts.next();
                let x = parts.next().unwrap().parse().unwrap();
                let rest = parts.next().unwrap();
                let literal = &rest[rest.find('(').unwrap() + 1..rest.rfind(") Tj").unwrap()];
                out.push(Drawn {
                    page,
                    font,
                    size,
                    x,
                    bytes: unescape(literal),
                });
            }
        }
        out
    }

    fn unescape(literal: &str) -> Vec<u8> {
        let bytes = literal.as_bytes();
        let mut out = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\\' {
                if bytes[index + 1].is_ascii_digit() {
                    let octal = std::str::from_utf8(&bytes[index + 1..index + 4]).unwrap();
                    out.push(u8::from_str_radix(octal, 8).unwrap());
                    index += 4;
                } else {
                    out.push(bytes[index + 1]);
                    index += 2;
                }
            } else {
                out.push(bytes[index]);
                index += 1;
            }
        }
        out
    }

    fn width_of(bytes: &[u8], size: f32) -> f32 {
        bytes
            .iter()
            .map(|byte| f32::from(REGULAR_WIDTHS[usize::from(byte - 32)]))
            .sum::<f32>()
            * size
            / 1000.0
    }

    fn compact(bytes: &[u8]) -> Vec<u8> {
        bytes.iter().copied().filter(|byte| *byte != b' ').collect()
    }

    fn report(rows: Vec<Vec<String>>) -> Report {
        Report {
            title: "Historique des mouvements".into(),
            scope: "Tous les mouvements".into(),
            sheet_name: "Historique".into(),
            quantity_columns: vec![4],
            headers: [
                "Date",
                "Produit",
                "Référence",
                "Mouvement",
                "Quantité",
                "Motif",
                "Référence documentaire",
                "Commentaire",
                "Source",
                "Inventaire",
            ]
            .map(String::from)
            .to_vec(),
            rows,
        }
    }

    #[test]
    fn les_chasses_sont_celles_d_helvetica() {
        // H 722, e 556, l 222, l 222, o 556 (AFM Adobe).
        assert_eq!(text_width("Hello", Font::Regular, 1000.0), 2278.0);
        assert_eq!(text_width("é", Font::Regular, 1000.0), 556.0);
        assert_eq!(text_width("m²", Font::Regular, 1000.0), 833.0 + 333.0);
        assert_eq!(text_width("A", Font::Bold, 1000.0), 722.0);
    }

    #[test]
    fn les_cellules_longues_sont_ecrites_en_entier_dans_leur_colonne() {
        let name = "Mortier-colle amélioré déformable C2S1 pour carrelage grand format, \
                    façades et piscines — sac de 25 kg, gris clair (palette de 48 sacs)";
        let reference = "REF-".to_string() + &"X".repeat(60);
        let comment = "Livraison partielle ; reliquat attendu. ".repeat(80);
        let mut rows = Vec::new();
        for index in 0..60 {
            rows.push(vec![
                "15/10/2026 08:30:00".to_string(),
                format!("{name} n° {index}"),
                reference.clone(),
                "Entrée".into(),
                "12 345,678".into(),
                "Réception fournisseur".into(),
                "BL-2026-000412".into(),
                if index == 7 {
                    comment.clone()
                } else {
                    String::new()
                },
                "Saisie".into(),
                String::new(),
            ]);
        }
        let report = report(rows);
        let widths = column_widths(&report.headers, &report.rows);
        assert!((widths.iter().sum::<f32>() - (PAGE_WIDTH - MARGIN * 2.0)).abs() < 0.5);
        let starts = widths
            .iter()
            .scan(MARGIN, |x, width| {
                let start = *x;
                *x += width;
                Some(start)
            })
            .collect::<Vec<_>>();
        let pages = layout(&report);
        assert!(pages.len() > 2, "{} pages", pages.len());
        let body = drawn(&pages)
            .into_iter()
            .filter(|text| text.font == "F1" && text.size == BODY_SIZE)
            .collect::<Vec<_>>();
        assert!(!body.is_empty());
        for (column, start) in starts.iter().enumerate() {
            let end = start + widths[column];
            let mut written = Vec::new();
            for text in body
                .iter()
                .filter(|text| (text.x - (start + CELL_PADDING)).abs() < 0.11)
            {
                // Chaque ligne tient dans sa colonne, sans déborder sur la suivante.
                assert!(
                    text.x + width_of(&text.bytes, text.size) <= end - CELL_PADDING + 0.1,
                    "colonne {column} : {:?}",
                    String::from_utf8_lossy(&text.bytes)
                );
                written.extend(compact(&text.bytes));
            }
            // Rien n'est tronqué : le contenu écrit est exactement celui du rapport.
            let expected = report
                .rows
                .iter()
                .flat_map(|row| compact(&winansi(&row[column])))
                .collect::<Vec<_>>();
            assert_eq!(
                String::from_utf8_lossy(&written),
                String::from_utf8_lossy(&expected),
                "colonne {}",
                report.headers[column]
            );
        }
        // Les en-têtes sont répétés sur chaque page.
        for page in 0..pages.len() {
            assert!(
                drawn(&pages)
                    .iter()
                    .any(|text| text.page == page && text.font == "F2" && text.bytes == b"Date"),
                "en-têtes absents de la page {}",
                page + 1
            );
        }
    }

    #[test]
    fn le_titre_et_le_perimetre_longs_passent_a_la_ligne() {
        let mut report = report(Vec::new());
        report.scope = "Mouvements du 01/01/2026 au 31/12/2026, produit « Mortier-colle \
                        amélioré déformable C2S1 », recherche « façade », tous les sens, \
                        toutes les sources, y compris les corrections d'inventaire validées \
                        par le responsable du dépôt principal de GTM Bâtiment."
            .into();
        let pages = layout(&report);
        let scope = drawn(&pages)
            .into_iter()
            .filter(|text| text.font == "F1" && text.size == 9.0 && text.bytes != b"Aucune ligne.")
            .skip(1)
            .collect::<Vec<_>>();
        assert!(scope.len() > 1);
        let written = scope
            .iter()
            .flat_map(|text| compact(&text.bytes))
            .collect::<Vec<_>>();
        assert_eq!(written, compact(&winansi(&report.scope)));
        for text in &scope {
            assert!(text.x + width_of(&text.bytes, 9.0) <= PAGE_WIDTH - MARGIN + 0.1);
        }
    }
}

const PROMPT_MARKERS: [&str; 2] = ["\u{276f}", ">"];
const BORDER: &str = "\u{2502}";

pub struct SuggestionScanner {
    parser: vt100::Parser,
    last: Option<String>,
}

impl SuggestionScanner {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            last: None,
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.parser.screen_mut().set_size(rows, cols);
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Option<Option<String>> {
        self.parser.process(bytes);
        let now = extract(self.parser.screen());
        if now == self.last {
            return None;
        }
        self.last = now.clone();
        Some(now)
    }

    pub fn current(&self) -> Option<String> {
        self.last.clone()
    }
}

pub fn extract(screen: &vt100::Screen) -> Option<String> {
    let (rows, cols) = screen.size();
    (0..rows)
        .rev()
        .find_map(|row| input_line(screen, row, cols))
        .flatten()
}

fn input_line(screen: &vt100::Screen, row: u16, cols: u16) -> Option<Option<String>> {
    let cells: Vec<&vt100::Cell> = (0..cols).filter_map(|c| screen.cell(row, c)).collect();
    let mut at = cells.iter().position(|c| !c.contents().trim().is_empty())?;
    if cells[at].contents() == BORDER {
        at += 1;
        while cells
            .get(at)
            .is_some_and(|c| c.contents().trim().is_empty())
        {
            at += 1;
        }
    }
    if !PROMPT_MARKERS.contains(&cells.get(at)?.contents()) {
        return None;
    }
    let mut text = String::new();
    for cell in &cells[at + 1..] {
        let glyph = cell.contents();
        if glyph.trim().is_empty() {
            text.push(' ');
            continue;
        }
        if glyph == BORDER {
            break;
        }
        let typed =
            !cell.dim() && !cell.inverse() && matches!(cell.fgcolor(), vt100::Color::Default);
        if typed {
            return Some(None);
        }
        text.push_str(glyph);
    }
    let text = text.trim();
    Some((!text.is_empty()).then(|| text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(bytes: &[u8]) -> Option<String> {
        let mut scanner = SuggestionScanner::new(80, 12);
        scanner.feed(bytes);
        scanner.current()
    }

    const RULE: &str = "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}";

    #[test]
    fn a_dim_placeholder_after_the_prompt_is_the_suggestion() {
        let screen = format!(
            "\u{23fa} Done.\r\n{RULE}\r\n\u{276f} \x1b[7mr\x1b[0m\x1b[2mun the tests\x1b[0m\r\n{RULE}\r\n  ? for shortcuts"
        );
        assert_eq!(read(screen.as_bytes()).as_deref(), Some("run the tests"));
    }

    #[test]
    fn a_boxed_prompt_with_a_gray_placeholder_reads_too() {
        let screen = "\u{256d}----\u{256e}\r\n\u{2502} > \x1b[90mcommit this\x1b[0m   \u{2502}\r\n\u{2570}----\u{256f}";
        assert_eq!(read(screen.as_bytes()).as_deref(), Some("commit this"));
    }

    #[test]
    fn typed_text_is_not_a_suggestion() {
        let screen = format!("{RULE}\r\n\u{276f} fix the bug\x1b[7m \x1b[0m\r\n{RULE}");
        assert_eq!(read(screen.as_bytes()), None);
    }

    #[test]
    fn an_empty_box_has_no_suggestion() {
        let screen = format!("{RULE}\r\n\u{276f} \x1b[7m \x1b[0m\r\n{RULE}");
        assert_eq!(read(screen.as_bytes()), None);
    }

    #[test]
    fn feed_reports_only_changes() {
        let mut scanner = SuggestionScanner::new(80, 6);
        assert_eq!(
            scanner.feed(b"\x1b[3;1H\xe2\x9d\xaf \x1b[2mgo on\x1b[0m"),
            Some(Some("go on".to_string()))
        );
        assert_eq!(scanner.feed(b""), None);
        assert_eq!(scanner.feed(b"\x1b[3;3H\x1b[Kx"), Some(None));
    }
}

use crate::Entry;

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind},
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{
        Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
    Frame,
};

pub struct App {
    header: apob::ApobHeader,
    items: Vec<Entry>,
    item_state: ratatui::widgets::TableState,
    item_scroll_state: ratatui::widgets::ScrollbarState,
    data_state: ratatui::widgets::TableState,
    data_scroll_state: ratatui::widgets::ScrollbarState,
}

impl App {
    pub fn new(header: apob::ApobHeader, items: Vec<Entry>) -> Self {
        Self {
            item_state: TableState::default().with_selected(0),
            item_scroll_state: ScrollbarState::new(items.len()),
            data_state: TableState::default().with_selected(0),
            data_scroll_state: ScrollbarState::new(1),
            items,
            header,
        }
    }

    pub fn run(mut self, mut terminal: ratatui::DefaultTerminal) {
        ratatui::crossterm::execute!(
            std::io::stdout(),
            ratatui::crossterm::event::EnableMouseCapture
        )
        .unwrap();
        loop {
            terminal.draw(|frame| self.draw(frame)).unwrap();
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                        KeyCode::Char('k') | KeyCode::Up => self.prev_row(),
                        _ => (),
                    }
                }
                Ok(Event::Mouse(m)) if m.kind == MouseEventKind::ScrollDown => {
                    self.next_row()
                }
                Ok(Event::Mouse(m)) if m.kind == MouseEventKind::ScrollUp => {
                    self.prev_row()
                }
                Ok(..) => (),
                Err(_) => break,
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let cols =
            &Layout::horizontal([Constraint::Length(45), Constraint::Fill(1)]);
        let rects = cols.split(frame.area());
        self.render_table(frame, rects[0]);
        self.render_item_scrollbar(frame, rects[0]);
        self.render_data(frame, rects[1]);
        self.render_data_scrollbar(frame, rects[1]);
    }

    fn render_data(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default().add_modifier(Modifier::BOLD);
        let selected_row_style =
            Style::default().add_modifier(Modifier::REVERSED);

        let width = 16; // TODO select based on terminal size
        let header = std::iter::once(Cell::from("OFFSET"))
            .chain((0..width).map(|i| Cell::from(format!("{i:02x}"))))
            .collect::<Row>()
            .style(header_style)
            .height(1);
        let Some(i) = self.item_state.selected() else {
            return;
        };
        let rows =
            self.items[i].data.chunks(width).enumerate().map(|(o, c)| {
                let offset = o * width;
                std::iter::once(Cell::from(format!("{:06x}", offset)))
                    .chain(c.iter().map(|b| Cell::from(format!("{b:02x}"))))
                    .chain(
                        std::iter::repeat(Cell::from("")).take(width - c.len()),
                    )
                    .chain(std::iter::once(
                        c.iter()
                            .map(|b| {
                                if b.is_ascii() && !b.is_ascii_control() {
                                    *b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect::<String>()
                            .into(),
                    ))
                    .collect::<Row>()
            });

        let t = Table::new(
            rows,
            std::iter::once(Constraint::Length(8))
                .chain((0..16).map(|_| Constraint::Length(2)))
                .chain(std::iter::once(Constraint::Length(
                    u16::try_from(width).unwrap(),
                ))),
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .block(Block::new().borders(Borders::ALL));

        frame.render_stateful_widget(t, area, &mut self.data_state);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default().add_modifier(Modifier::BOLD);
        let selected_row_style =
            Style::default().add_modifier(Modifier::REVERSED);

        let header = ["OFFSET", "GROUP", "TYPE", "INSTANCE", "DATA SIZE"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);
        let cf = |t| Cell::from(Text::from(t));
        let cfl = |t| Cell::from(Text::from(t).alignment(Alignment::Right));
        let rows = self.items.iter().map(|item| {
            let entry = &item.entry;
            let group = entry.group().unwrap();
            let color = match group {
                apob::ApobGroup::MEMORY => Color::Blue,
                apob::ApobGroup::DF => Color::LightBlue,
                apob::ApobGroup::CCX => Color::Red,
                apob::ApobGroup::NBIO => Color::LightGreen,
                apob::ApobGroup::FCH => Color::LightRed,
                apob::ApobGroup::PSP => Color::LightCyan,
                apob::ApobGroup::GENERAL => Color::Magenta,
                apob::ApobGroup::SMBIOS => Color::Green,
                apob::ApobGroup::FABRIC => Color::Cyan,
                apob::ApobGroup::APCB => Color::LightMagenta,
            };
            [
                cfl(format!("{:05x}", item.offset)),
                cf(format!("{:?}", group)).style(Style::default().fg(color)),
                cfl(format!("{:x}", entry.ty & !apob::APOB_CANCELLED)),
                cfl(format!("{:x}", entry.inst)),
                cfl(format!(
                    "{:x}",
                    entry.size as usize - std::mem::size_of_val(entry)
                )),
            ]
            .into_iter()
            .collect::<Row>()
            .style(Style::new())
            .height(1)
        });
        let t = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(4),
                Constraint::Length(8),
                Constraint::Length(9),
            ],
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .block(Block::new().borders(Borders::ALL));

        frame.render_stateful_widget(t, area, &mut self.item_state);
    }

    fn render_item_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::reset()),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.item_scroll_state,
        );
    }

    fn render_data_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::reset()),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.data_scroll_state,
        );
    }

    pub fn next_row(&mut self) {
        let i = match self.item_state.selected() {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        };
        self.set_item_scroll(i);
    }

    pub fn prev_row(&mut self) {
        let i = match self.item_state.selected() {
            Some(i) => i.checked_sub(1).unwrap_or_else(|| self.items.len() - 1),
            None => 0,
        };
        self.set_item_scroll(i);
    }

    fn set_item_scroll(&mut self, i: usize) {
        self.item_state.select(Some(i));
        self.item_scroll_state = self.item_scroll_state.position(i);
        self.data_state.select(Some(0));
        self.data_scroll_state =
            ScrollbarState::new(self.items[i].data.len().div_ceil(16));
    }
}

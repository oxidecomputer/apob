use crate::Entry;

use std::collections::HashMap;

use ratatui::{
    crossterm::event::{
        self, Event, KeyCode, KeyEventKind, MouseButton, MouseEvent,
        MouseEventKind,
    },
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
    data_scroll_cache: HashMap<usize, usize>,
    data_scroll_max: usize,
    data_width: usize,
    data_focus: bool,
    window_height: u16,
}

impl App {
    pub fn new(header: apob::ApobHeader, items: Vec<Entry>) -> Self {
        let mut out = Self {
            item_state: TableState::default().with_selected(0),
            item_scroll_state: ScrollbarState::new(items.len()),
            data_state: TableState::default().with_selected(0),
            data_scroll_state: ScrollbarState::new(1),
            data_scroll_cache: HashMap::new(),
            data_scroll_max: 1,
            data_width: 8,
            data_focus: false,
            window_height: 16,
            items,
            header,
        };
        out.set_item_scroll(0);
        out
    }

    pub fn run(mut self, mut terminal: ratatui::DefaultTerminal) {
        ratatui::crossterm::execute!(
            std::io::stdout(),
            ratatui::crossterm::event::EnableMouseCapture
        )
        .unwrap();
        loop {
            terminal.draw(|frame| self.draw(frame)).unwrap();
            let e = event::read();
            // Use the mouse to set focus in one pane or the other
            if let Ok(Event::Mouse(m)) = &e {
                self.data_focus = m.column > 45;
            }
            match e {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('0') => {
                            if self.data_focus {
                                self.set_data_scroll(0)
                            } else {
                                self.set_item_scroll(0)
                            }
                        }
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('j') | KeyCode::Down => {
                            if self.data_focus {
                                self.next_data_row(1)
                            } else {
                                self.next_item_row(1)
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if self.data_focus {
                                self.prev_data_row(1)
                            } else {
                                self.prev_item_row(1)
                            }
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            self.data_focus = true;
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            self.data_focus = false;
                        }
                        KeyCode::PageDown => {
                            if self.data_focus {
                                self.next_data_row(self.window_height.into())
                            } else {
                                self.next_item_row(self.window_height.into())
                            }
                        }
                        KeyCode::PageUp => {
                            if self.data_focus {
                                self.prev_data_row(self.window_height.into())
                            } else {
                                self.prev_item_row(self.window_height.into())
                            }
                        }
                        _ => (),
                    }
                }
                Ok(Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    row,
                    ..
                })) if !self.data_focus => {
                    let i = self.item_state.offset();
                    if let Some(sel) = (i + usize::from(row)).checked_sub(2) {
                        if sel < self.items.len() {
                            self.set_item_scroll(sel);
                        }
                    }
                }
                Ok(Event::Mouse(m)) if m.kind == MouseEventKind::ScrollDown => {
                    if self.data_focus {
                        self.next_data_row(1)
                    } else {
                        self.next_item_row(1)
                    }
                }
                Ok(Event::Mouse(m)) if m.kind == MouseEventKind::ScrollUp => {
                    if self.data_focus {
                        self.prev_data_row(1)
                    } else {
                        self.prev_item_row(1)
                    }
                }
                Ok(..) => (),
                Err(_) => break,
            }
        }
        ratatui::crossterm::execute!(
            std::io::stdout(),
            ratatui::crossterm::event::DisableMouseCapture
        )
        .unwrap();
    }

    fn draw(&mut self, frame: &mut Frame) {
        let cols =
            &Layout::horizontal([Constraint::Length(45), Constraint::Fill(1)]);
        let rects = cols.split(frame.area());
        self.render_table(frame, rects[0], !self.data_focus);
        self.render_data(frame, rects[1], self.data_focus);

        self.window_height = rects[0].height.saturating_sub(3);
    }

    fn resize_data(&mut self, data_width: usize) {
        if data_width != self.data_width {
            for (_, row) in self.data_scroll_cache.iter_mut() {
                let index = *row * self.data_width;
                *row = index / data_width;
            }
            if let Some(row) = self.data_state.selected() {
                let index = row * self.data_width;
                self.set_data_scroll(index / data_width);
            }
            self.data_width = data_width;
        }
    }

    fn render_data(&mut self, frame: &mut Frame, area: Rect, focus: bool) {
        let header_style = Style::new().add_modifier(Modifier::BOLD);
        let selected_row_style = Style::new().add_modifier(Modifier::REVERSED);

        const OFFSET_COL: u16 = 8;
        let available_width = area.width - 3;
        let width = if available_width >= OFFSET_COL + 1 + 16 * 3 + 16 {
            16
        } else {
            8
        };
        self.resize_data(width);

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
            std::iter::once(Constraint::Length(OFFSET_COL))
                .chain((0..width).map(|_| Constraint::Length(2)))
                .chain(std::iter::once(Constraint::Length(
                    u16::try_from(width).unwrap(),
                ))),
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(Self::border_style(focus)),
        );

        frame.render_stateful_widget(t, area, &mut self.data_state);

        // Draw the scroll bar
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Self::scrollbar_style(focus)),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.data_scroll_state,
        );
    }

    fn border_style(focus: bool) -> Style {
        if focus {
            Style::new()
        } else {
            Style::new().add_modifier(Modifier::DIM)
        }
    }

    fn scrollbar_style(focus: bool) -> Style {
        if focus {
            Style::reset()
        } else {
            Style::reset().add_modifier(Modifier::DIM)
        }
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, focus: bool) {
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
            let cancelled = entry.cancelled();
            let style = if cancelled {
                Style::new().add_modifier(Modifier::DIM)
            } else {
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
                Style::new().fg(color)
            };
            [
                cfl(format!("{:05x}", item.offset)),
                cf(format!("{:?}{}", group, if cancelled { "*" } else { "" }))
                    .style(style),
                cfl(format!("{:x}", entry.ty & !apob::APOB_CANCELLED)),
                cfl(format!("{:x}", entry.inst)),
                cfl(format!(
                    "{:x}",
                    entry.size as usize - std::mem::size_of_val(entry)
                )),
            ]
            .into_iter()
            .collect::<Row>()
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
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(Self::border_style(focus)),
        );

        frame.render_stateful_widget(t, area, &mut self.item_state);

        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Self::scrollbar_style(focus)),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.item_scroll_state,
        );
    }

    pub fn next_item_row(&mut self, d: usize) {
        let i = match self.item_state.selected() {
            Some(i) => (i + d).min(self.items.len() - 1),
            None => 0,
        };
        self.set_item_scroll(i);
    }

    pub fn prev_item_row(&mut self, d: usize) {
        let i = match self.item_state.selected() {
            Some(i) => i.saturating_sub(d),
            None => 0,
        };
        self.set_item_scroll(i);
    }

    fn set_item_scroll(&mut self, i: usize) {
        self.item_state.select(Some(i));
        self.item_scroll_state = self.item_scroll_state.position(i);
        self.data_state
            .select(Some(self.data_scroll_cache.get(&i).cloned().unwrap_or(0)));
        self.data_scroll_max = self.items[i].data.len().div_ceil(16);
        self.data_scroll_state = ScrollbarState::new(self.data_scroll_max);
    }

    pub fn next_data_row(&mut self, d: usize) {
        let i = match self.data_state.selected() {
            Some(i) => (i + d).min(self.data_scroll_max - 1),
            None => 0,
        };
        self.set_data_scroll(i);
    }

    pub fn prev_data_row(&mut self, d: usize) {
        let i = match self.data_state.selected() {
            Some(i) => i.saturating_sub(d),
            None => 0,
        };
        self.set_data_scroll(i);
    }

    pub fn set_data_scroll(&mut self, i: usize) {
        if let Some(j) = self.item_state.selected() {
            self.data_scroll_cache.insert(j, i);
        }
        self.data_state.select(Some(i));
        self.data_scroll_state = self.data_scroll_state.position(i);
    }
}

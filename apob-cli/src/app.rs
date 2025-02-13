use crate::Entry;

use std::collections::HashMap;

use ratatui::{
    crossterm::event::{
        self, Event, KeyCode, KeyEventKind, MouseButton, MouseEvent,
        MouseEventKind,
    },
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
    Frame,
};
use zerocopy::FromBytes;

enum DataGrouping {
    Byte,
    Word,
    DoubleWord,
}

enum Endian {
    Little,
    Big,
}

#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(name(SpecializedTag))]
enum SpecializedState {
    ApobEventLog(TableState),
    ApobMemMap(TableState),
}

impl DataGrouping {
    fn bytes(&self) -> usize {
        match self {
            DataGrouping::Byte => 1,
            DataGrouping::Word => 2,
            DataGrouping::DoubleWord => 4,
        }
    }
}

pub struct App {
    items: Vec<Entry>,
    item_state: TableState,
    data_state: TableState,
    data_scroll_cache: HashMap<usize, usize>,
    data_scroll_max: usize,
    data_width: usize,
    data_endian: Endian,
    data_focus: bool,
    data_grouping: DataGrouping,
    specialized_state: Option<SpecializedState>,
    window_height: u16,
}

impl App {
    pub fn new(items: Vec<Entry>) -> Self {
        let mut out = Self {
            item_state: TableState::default().with_selected(0),
            data_state: TableState::default().with_selected(0),
            data_scroll_cache: HashMap::new(),
            data_scroll_max: 1,
            data_grouping: DataGrouping::Byte,
            data_width: 8,
            data_endian: Endian::Little,
            data_focus: false,
            specialized_state: None,
            window_height: 16,
            items,
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
        let mut scroll_momentum = 1;
        loop {
            terminal.draw(|frame| self.draw(frame)).unwrap();
            let event_was_ready =
                event::poll(std::time::Duration::from_millis(50))
                    .unwrap_or(false);
            let e = event::read();
            // Use the mouse to set focus in one pane or the other
            if let Ok(Event::Mouse(m)) = &e {
                self.data_focus = m.column > 45;
            }
            let mut reset_momentum = true;
            if !event_was_ready {
                scroll_momentum = 1;
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
                        KeyCode::Char('1') => {
                            self.data_grouping = DataGrouping::Byte
                        }
                        KeyCode::Char('2') => {
                            self.data_grouping = DataGrouping::Word
                        }
                        KeyCode::Char('4') => {
                            self.data_grouping = DataGrouping::DoubleWord
                        }
                        KeyCode::Char('e') => {
                            self.data_endian = match self.data_endian {
                                Endian::Big => Endian::Little,
                                Endian::Little => Endian::Big,
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
                        self.next_data_row(scroll_momentum)
                    } else {
                        self.next_item_row(scroll_momentum)
                    }
                    if event_was_ready {
                        reset_momentum = false;
                        scroll_momentum = (scroll_momentum + 1).min(16);
                    }
                }
                Ok(Event::Mouse(m)) if m.kind == MouseEventKind::ScrollUp => {
                    if self.data_focus {
                        self.prev_data_row(scroll_momentum)
                    } else {
                        self.prev_item_row(scroll_momentum)
                    }
                    if event_was_ready {
                        reset_momentum = false;
                        scroll_momentum = (scroll_momentum + 1).min(16);
                    }
                }
                Ok(..) => (),
                Err(_) => break,
            }
            if reset_momentum {
                scroll_momentum = 1;
            }
        }
        ratatui::crossterm::execute!(
            std::io::stdout(),
            ratatui::crossterm::event::DisableMouseCapture
        )
        .unwrap();
    }

    /// Checks whether we have a specialized drawing algorithm for this entry
    fn specialized(h: apob::ApobEntry) -> Option<SpecializedTag> {
        match (h.group(), h.ty) {
            (Some(apob::ApobGroup::GENERAL), 6) => {
                Some(SpecializedTag::ApobEventLog)
            }
            (Some(apob::ApobGroup::FABRIC), t)
                if t == apob::ApobFabricType::SYS_MEM_MAP as u32 =>
            {
                Some(SpecializedTag::ApobMemMap)
            }
            _ => None,
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let cols =
            &Layout::horizontal([Constraint::Length(45), Constraint::Fill(1)]);
        let rects = cols.split(frame.area());
        self.window_height = rects[0].height.saturating_sub(3);
        self.render_table(frame, rects[0], !self.data_focus);

        let specialized = self
            .item_state
            .selected()
            .and_then(|i| Self::specialized(self.items[i].entry));

        let rows = if specialized.is_some() {
            Layout::vertical([
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
        } else {
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)])
        };
        let rects = rows.split(rects[1]);
        self.render_data(frame, rects[0], self.data_focus);

        if let Some(s) = specialized {
            self.render_specialized(s, frame, rects[1]);
        } else {
            self.clear_specialized();
        }

        let help = Span::raw(format!(
            " [{}]-byte groups, {}-[e]ndian",
            self.data_grouping.bytes(),
            match self.data_endian {
                Endian::Big => "big",
                Endian::Little => "little",
            }
        ));
        frame.render_widget(help, *rects.last().unwrap());
    }

    fn render_specialized(
        &mut self,
        s: SpecializedTag,
        frame: &mut Frame,
        rect: Rect,
    ) {
        let needs_reset =
            self.specialized_state.as_ref().map(SpecializedTag::from)
                != Some(s);
        let entry = &self.items[self.item_state.selected().unwrap()];
        if needs_reset {
            self.specialized_state = Some(match s {
                SpecializedTag::ApobMemMap => {
                    SpecializedState::ApobMemMap(TableState::new())
                }
                SpecializedTag::ApobEventLog => {
                    SpecializedState::ApobEventLog(TableState::new())
                }
            })
        }

        let header_style = Style::default().add_modifier(Modifier::BOLD);
        let selected_row_style = Style::new().add_modifier(Modifier::REVERSED);

        match self.specialized_state.as_mut().unwrap() {
            SpecializedState::ApobMemMap(data) => {
                let header = ["BASE", "SIZE", "TYPE"]
                    .into_iter()
                    .map(Cell::from)
                    .collect::<Row>()
                    .style(header_style);
                let (map, holes) =
                    apob::ApobSysMemMap::ref_from_prefix(&entry.data).unwrap();
                let holes =
                    <[apob::ApobSysMemMapHole]>::ref_from_bytes(holes).unwrap();

                let holes = holes[..map.hole_count as usize].iter().map(|h| {
                    [
                        format!("0x{:0>10x}", h.base),
                        format!("0x{:0>8x}", h.size),
                        format!("{:#04x}", h.ty),
                    ]
                    .into_iter()
                    .map(Cell::from)
                    .collect::<Row>()
                });

                let border_set = ratatui::symbols::border::Set {
                    top_left: ratatui::symbols::line::NORMAL.vertical_right,
                    top_right: ratatui::symbols::line::NORMAL.vertical_left,
                    ..ratatui::symbols::border::PLAIN
                };
                let outer = Block::new()
                    .borders(Borders::ALL)
                    .title("APOB memory map")
                    .title_style(header_style); // TODO focus
                frame.render_widget(outer, rect);

                let header_rect = rect.inner(Margin::new(1, 1));
                frame.render_widget(
                    Span::from(format!("high_phys: {:#x}", map.high_phys)),
                    header_rect,
                );

                let mut rect = rect;
                rect.y += 3;
                rect.height -= 3;
                let t = Table::new(
                    holes,
                    [
                        Constraint::Length(14),
                        Constraint::Length(12),
                        Constraint::Length(8),
                    ],
                )
                .header(header)
                .row_highlight_style(selected_row_style)
                .block(
                    Block::new()
                        .borders(Borders::ALL)
                        .border_set(border_set)
                        .title("HOLES")
                        .title_style(header_style),
                );

                frame.render_stateful_widget(t, rect, data);
            }
            SpecializedState::ApobEventLog(data) => {
                let header = ["INDEX", "CLASS", "EVENT", "DATA", ""]
                    .into_iter()
                    .map(Cell::from)
                    .collect::<Row>()
                    .style(header_style);
                let (log, _) =
                    apob::MilanApobEventLog::ref_from_prefix(&entry.data)
                        .unwrap();
                let log = log.events[..log.count as usize]
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        [
                            format!("{i:02x}"),
                            if let Some(c) =
                                apob::MilanApobEventClass::from_repr(
                                    v.class as usize,
                                )
                            {
                                format!("{c:?} ({:#x})", v.class)
                            } else {
                                format!("{:#x}", v.class)
                            },
                            format!("{:#x}", v.info),
                            format!("{:#x}", v.data0),
                            format!("{:#x}", v.data1),
                        ]
                        .into_iter()
                        .map(Cell::from)
                        .collect::<Row>()
                    });

                let t = Table::new(
                    log,
                    [
                        Constraint::Length(7),
                        Constraint::Length(14),
                        Constraint::Length(9),
                        Constraint::Length(10),
                        Constraint::Length(10),
                    ],
                )
                .header(header)
                .row_highlight_style(selected_row_style)
                .block(
                    Block::new()
                        .borders(Borders::ALL)
                        .title("APOB event log")
                        .title_style(header_style), // TODO focus
                );

                frame.render_stateful_widget(t, rect, data);
            }
        };
    }

    fn clear_specialized(&mut self) {
        self.specialized_state = None;
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

        let bs = self.data_grouping.bytes();
        let header = std::iter::once(Cell::from("OFFSET"))
            .chain(
                (0..width / bs).map(|i| Cell::from(format!("{:02x}", i * bs))),
            )
            .collect::<Row>()
            .style(header_style);
        let Some(i) = self.item_state.selected() else {
            return;
        };
        let rows =
            self.items[i].data.chunks(width).enumerate().map(|(o, c)| {
                let offset = o * width;
                std::iter::once(
                    Line::from(format!("{:06x}", offset))
                        .style(Style::new().add_modifier(Modifier::DIM))
                        .into(),
                )
                .chain(c.chunks(bs).map(|c| {
                    let mut s = String::new();
                    match self.data_endian {
                        Endian::Little => {
                            for b in c.iter().rev() {
                                s += &format!("{b:02x}");
                            }
                        }
                        Endian::Big => {
                            for b in c.iter() {
                                s += &format!("{b:02x}");
                            }
                        }
                    }
                    Cell::from(s)
                }))
                .chain(
                    // Empty cells to fill out the remaining size
                    std::iter::repeat(Cell::from(""))
                        .take(width / bs - c.len() / bs),
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
                .chain((0..width / bs).map(|_| {
                    Constraint::Length(u16::try_from(bs * 2).unwrap())
                }))
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
        if let Some(i) = self.data_state.selected() {
            let mut data_scroll_state =
                ScrollbarState::new(self.items.len()).position(i);
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
                &mut data_scroll_state,
            );
        }
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
            .style(header_style);
        let cf = |t| Cell::from(Span::from(t));
        let cfl = |t| Cell::from(Line::from(t).alignment(Alignment::Right));
        let rows = self.items.iter().map(|item| {
            let entry = &item.entry;
            let group = entry.group().unwrap();
            let cancelled = entry.cancelled();
            let group_style = if cancelled {
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
            let specialized = Self::specialized(*entry).is_some();
            [
                cfl(format!("{:05x}", item.offset)),
                cf(format!(
                    "{:?}{}",
                    group,
                    if cancelled {
                        "*"
                    } else if specialized {
                        "+"
                    } else {
                        ""
                    }
                ))
                .style(group_style),
                cfl(format!("{:x}", entry.ty & !apob::APOB_CANCELLED)),
                cfl(format!("{:x}", entry.inst)),
                cfl(format!(
                    "{:x}",
                    entry.size as usize - std::mem::size_of_val(entry)
                )),
            ]
            .into_iter()
            .collect::<Row>()
        });

        let t = Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(8),
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

        // Draw the scroll bar
        if let Some(i) = self.item_state.selected() {
            let mut item_scroll_state =
                ScrollbarState::new(self.items.len()).position(i);
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
                &mut item_scroll_state,
            );
        }
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
        self.data_state
            .select(Some(self.data_scroll_cache.get(&i).cloned().unwrap_or(0)));
        self.data_scroll_max =
            self.items[i].data.len().div_ceil(self.data_width);
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
    }
}

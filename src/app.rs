use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;
use crate::scanner::ComObject;
use crate::error_handling::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Scanning,
    Browsing,
    Inspecting,
}

pub struct App {
    pub objects_list: Vec<ComObject>,
    pub search_query: String,
    pub list_state: ListState,
    pub app_mode: AppMode,
    pub should_quit: bool,
}

impl App {
    pub fn new(objects: Vec<ComObject>) -> Self {
        let mut list_state = ListState::default();
        if !objects.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            objects_list: objects,
            search_query: String::new(),
            list_state,
            app_mode: AppMode::Browsing,
            should_quit: false,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // terminal.draw takes a closure. We need to pass mutable state.
            // We split the borrow here implicitly by passing fields.
            terminal.draw(|f| ui_render(f, &self.objects_list, &self.app_mode, &mut self.list_state))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => self.should_quit = true,
                            KeyCode::Down => self.next(),
                            KeyCode::Up => self.previous(),
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn next(&mut self) {
        if self.objects_list.is_empty() {
            return;
        }
        
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.objects_list.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.objects_list.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.objects_list.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

fn ui_render(f: &mut Frame, objects: &[ComObject], mode: &AppMode, list_state: &mut ListState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[0]);

    // Left Pane: Object List
    let items: Vec<ListItem> = objects
        .iter()
        .map(|obj| {
            let content = format!("{} ({})", obj.name, obj.clsid);
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("COM Objects"))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    
    f.render_stateful_widget(list, main_chunks[0], list_state);

    // Right Pane: Details
    let selected_index = list_state.selected();
    let details_text = if let Some(idx) = selected_index {
        if let Some(obj) = objects.get(idx) {
             vec![
                Line::from(Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD))),
                Line::from(obj.name.as_str()),
                Line::from(""),
                Line::from(Span::styled("CLSID: ", Style::default().add_modifier(Modifier::BOLD))),
                Line::from(obj.clsid.as_str()),
                Line::from(""),
                Line::from(Span::styled("Description: ", Style::default().add_modifier(Modifier::BOLD))),
                Line::from(obj.description.as_str()),
            ]
        } else {
             vec![Line::from("Selected index out of bounds")]
        }
    } else {
        vec![Line::from("No object selected")]
    };

    let details = Paragraph::new(details_text)
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(details, main_chunks[1]);

    // Bottom Bar
    let status_text = format!(
        "Mode: {:?} | Objects: {} | Press 'q' to quit", 
        mode, 
        objects.len()
    );
    let status = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[1]);
}
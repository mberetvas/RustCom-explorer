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
use crate::com_interop::{self, TypeDetails, Member, AccessMode};

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
    // New fields for state management
    pub selected_object: Option<TypeDetails>,
    pub error_message: Option<String>,
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
            selected_object: None,
            error_message: None,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| ui_render(f, self))?;

            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => self.should_quit = true,
                            KeyCode::Down => self.next(),
                            KeyCode::Up => self.previous(),
                            KeyCode::Enter => self.inspect_selected(),
                            KeyCode::Esc => self.exit_inspection(),
                            _ => {}
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

    fn inspect_selected(&mut self) {
        if let Some(index) = self.list_state.selected()
            && let Some(obj) = self.objects_list.get(index) {
                // Clear previous state
                self.selected_object = None;
                self.error_message = None;

                // Attempt to get type info (Note: Blocking operation)
                match com_interop::get_type_info(&obj.clsid) {
                    Ok(details) => {
                        self.selected_object = Some(details);
                    }
                    Err(e) => {
                        self.error_message = Some(e.to_string());
                    }
                }
                
                // Transition to Inspecting mode to show details/error in right pane
                self.app_mode = AppMode::Inspecting;
            }
    }

    fn exit_inspection(&mut self) {
        // Allow exiting inspection mode with Esc
        if self.app_mode == AppMode::Inspecting {
            self.app_mode = AppMode::Browsing;
            self.selected_object = None;
            self.error_message = None;
        }
    }
}

fn ui_render(f: &mut Frame, app: &mut App) {
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
    let items: Vec<ListItem> = app.objects_list
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
    
    // We access list_state mutably here, separate from objects_list borrow above
    f.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // Right Pane: Details
    let right_pane_block = Block::default()
        .borders(Borders::ALL)
        .title(match app.app_mode {
            AppMode::Inspecting => "Details (Inspecting)",
            _ => "Details",
        })
        .style(if app.app_mode == AppMode::Inspecting {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let details_text = match app.app_mode {
        AppMode::Inspecting => {
            if let Some(err_msg) = &app.error_message {
                vec![
                    Line::from(Span::styled("Error Inspecting Object:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(err_msg, Style::default().fg(Color::Red))),
                ]
            } else if let Some(details) = &app.selected_object {
                let mut lines = vec![
                    Line::from(Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD))),
                    Line::from(details.name.as_str()),
                    Line::from(""),
                    Line::from(Span::styled("Description: ", Style::default().add_modifier(Modifier::BOLD))),
                    Line::from(details.description.as_str()),
                    Line::from(""),
                    Line::from(Span::styled("Members:", Style::default().add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED))),
                ];

                if details.members.is_empty() {
                     lines.push(Line::from("No members found or type info unavailable."));
                } else {
                    for member in &details.members {
                        match member {
                            Member::Method { name, signature, return_type: _ } => {
                                lines.push(Line::from(vec![
                                    Span::styled("M ", Style::default().fg(Color::Cyan)), 
                                    Span::raw(format!("{}{}", name, signature))
                                ]));
                            },
                            Member::Property { name, value_type, access } => {
                                let access_badge = match access {
                                    AccessMode::Read => "R",
                                    AccessMode::Write => "W",
                                    AccessMode::ReadWrite => "RW",
                                };
                                lines.push(Line::from(vec![
                                    Span::styled("P ", Style::default().fg(Color::Green)),
                                    Span::styled(format!("[{}] ", access_badge), Style::default().fg(Color::DarkGray)),
                                    Span::raw(format!("{}: {}", name, value_type))
                                ]));
                            }
                        }
                    }
                }
                lines
            } else {
                 vec![Line::from("Loading...")]
            }
        },
        _ => {
            // Browsing Mode: Show basic metadata
            if let Some(idx) = app.list_state.selected() {
                if let Some(obj) = app.objects_list.get(idx) {
                    vec![
                        Line::from(Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD))),
                        Line::from(obj.name.as_str()),
                        Line::from(""),
                        Line::from(Span::styled("CLSID: ", Style::default().add_modifier(Modifier::BOLD))),
                        Line::from(obj.clsid.as_str()),
                        Line::from(""),
                        Line::from(Span::styled("Description: ", Style::default().add_modifier(Modifier::BOLD))),
                        Line::from(obj.description.as_str()),
                        Line::from(""),
                        Line::from(Span::styled("Hint: Press <Enter> to inspect details.", Style::default().fg(Color::Gray))),
                    ]
                } else {
                    vec![Line::from("Selected index out of bounds")]
                }
            } else {
                vec![Line::from("No object selected")]
            }
        }
    };

    let details = Paragraph::new(details_text)
        .block(right_pane_block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(details, main_chunks[1]);

    // Bottom Bar
    let status_text = format!(
        "Mode: {:?} | Objects: {} | <Up/Down>: Navigate | <Enter>: Inspect | <Esc>: Back | <q>: Quit", 
        app.app_mode, 
        app.objects_list.len()
    );
    let status = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[1]);
}
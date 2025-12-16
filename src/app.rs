use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
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

/// Helper function to filter objects based on a query.
/// Defined outside impl App to allow disjoint borrowing of App fields.
fn filter_objects<'a>(objects: &'a [ComObject], query: &str) -> Vec<&'a ComObject> {
    if query.is_empty() {
        objects.iter().collect()
    } else {
        let q = query.to_lowercase();
        objects.iter()
            .filter(|obj| 
                obj.name.to_lowercase().contains(&q) || 
                obj.clsid.to_lowercase().contains(&q) ||
                obj.description.to_lowercase().contains(&q)
            )
            .collect()
    }
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

    /// Helper function to get objects filtered by the search query.
    /// Note: usages of this method borrow the whole App instance.
    /// For disjoint field borrowing (like in ui_render), use the standalone filter_objects function.
    pub fn get_filtered_objects(&self) -> Vec<&ComObject> {
        filter_objects(&self.objects_list, &self.search_query)
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| ui_render(f, self))?;

            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Char(c) => {
                                if self.app_mode == AppMode::Browsing {
                                    self.search_query.push(c);
                                    // Reset selection to top when search changes
                                    if !self.get_filtered_objects().is_empty() {
                                        self.list_state.select(Some(0));
                                    } else {
                                        self.list_state.select(None);
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                if self.app_mode == AppMode::Browsing {
                                    let _ = self.search_query.pop();
                                     if !self.get_filtered_objects().is_empty() {
                                        self.list_state.select(Some(0));
                                    } else {
                                        self.list_state.select(None);
                                    }
                                }
                            }
                            KeyCode::Down => self.next(),
                            KeyCode::Up => self.previous(),
                            KeyCode::Enter => self.inspect_selected(),
                            KeyCode::Esc => {
                                if self.app_mode == AppMode::Inspecting {
                                    self.exit_inspection();
                                } else if !self.search_query.is_empty() {
                                    self.search_query.clear();
                                    self.list_state.select(Some(0));
                                }
                            }
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
        let filtered = self.get_filtered_objects();
        if filtered.is_empty() {
            return;
        }
        
        // Calculate new index using filtered list
        // We must drop 'filtered' before mutating self.list_state to satisfy borrow checker
        // if NLL doesn't handle it automatically (it should if we don't use filtered after).
        let new_idx = match self.list_state.selected() {
            Some(i) => {
                if i >= filtered.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        // filtered is no longer used here
        self.list_state.select(Some(new_idx));
    }

    fn previous(&mut self) {
        let filtered = self.get_filtered_objects();
        if filtered.is_empty() {
            return;
        }

        let new_idx = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    filtered.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(new_idx));
    }

    fn inspect_selected(&mut self) {
        // 1. Identify the object CLSID
        // We isolate the borrow of 'self' (via get_filtered_objects) to this block.
        // We clone the CLSID so we can drop the references to self.
        let clsid_opt = {
            let filtered = self.get_filtered_objects();
            self.list_state.selected()
                .and_then(|index| filtered.get(index))
                .map(|obj| obj.clsid.clone())
        };

        // 2. Perform the inspection (mutation of self)
        if let Some(clsid) = clsid_opt {
            // Clear previous state
            self.selected_object = None;
            self.error_message = None;

            // Attempt to get type info (Note: Blocking operation)
            match com_interop::get_type_info(&clsid) {
                Ok(details) => {
                    self.selected_object = Some(details);
                }
                Err(e) => {
                    self.error_message = Some(e.to_string());
                }
            }
            
            // Transition to Inspecting mode
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

    // Verified: 50/50 split ratio as per Task 2.3 requirements.
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[0]);

    // Left Pane: Object List (Filtered)
    // We use disjoint borrowing here to avoid E0502
    let objects_list = &app.objects_list;
    let search_query = &app.search_query;
    let filtered_objects = filter_objects(objects_list, search_query);

    let items: Vec<ListItem> = filtered_objects
        .iter()
        .map(|obj| {
            let content = format!("{} ({})", obj.name, obj.clsid);
            ListItem::new(content)
        })
        .collect();

    let list_title = if search_query.is_empty() {
        "COM Objects".to_string()
    } else {
        format!("COM Objects (Filter: '{}')", search_query)
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    
    // We access list_state mutably here. 
    // Since filtered_objects borrows from objects_list/search_query, and list_state is separate, this is safe.
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
            // Browsing Mode: Show basic metadata for selected item in filtered list
            // filtered_objects is still valid here (immutable borrow)
            if let Some(idx) = app.list_state.selected() {
                if let Some(obj) = filtered_objects.get(idx) {
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
    let current_selection_name = if let Some(idx) = app.list_state.selected() {
         filtered_objects.get(idx).map(|o| o.name.as_str()).unwrap_or("Unknown")
    } else {
        "None"
    };

    let mode_str = match app.app_mode {
        AppMode::Scanning => "SCANNING",
        AppMode::Browsing => "BROWSING",
        AppMode::Inspecting => "INSPECTING",
    };

    let search_status = if search_query.is_empty() {
        " [Search: <Type to Filter>]".to_string()
    } else {
        format!(" [Search: '{}']", search_query)
    };

    let status_text = format!(
        "Mode: {} | Obj: {} | Count: {}/{} |{} | <Up/Down>: Nav | <Enter>: Insp | <Esc>: Back/Clear | <Ctrl+c>: Quit", 
        mode_str,
        current_selection_name,
        filtered_objects.len(),
        app.objects_list.len(),
        search_status
    );
    let status = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[1]);
}
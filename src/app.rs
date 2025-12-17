// src/app.rs
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
use crate::error_handling::{Result, Context};
use crate::com_interop::{self, TypeDetails, Member, AccessMode};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use arboard::Clipboard;

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
    
    // State for Inspecting Mode
    pub selected_object: Option<TypeDetails>,
    pub error_message: Option<String>,
    pub inspection_receiver: Option<Receiver<Result<TypeDetails>>>,
    pub member_list_state: ListState,
    pub clipboard_status: Option<String>,
}

/// Helper function to filter objects based on a query.
fn filter_objects<'a>(objects: &'a [ComObject], query: &str) -> Vec<&'a ComObject> {
    if query.is_empty() {
        return objects.iter().collect();
    }

    let matcher = SkimMatcherV2::default();

    let mut scored: Vec<(i64, &'a ComObject)> = objects.iter()
        .filter_map(|obj| {
            let s_name = matcher.fuzzy_match(&obj.name, query).map(|s| s + 10);
            let s_clsid = matcher.fuzzy_match(&obj.clsid, query).map(|s| s + 5);
            let s_desc = matcher.fuzzy_match(&obj.description, query);

            let max_score = [s_name, s_clsid, s_desc]
                .iter()
                .filter_map(|&s| s)
                .max();

            max_score.map(|score| (score, obj))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, obj)| obj).collect()
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
            inspection_receiver: None,
            member_list_state: ListState::default(),
            clipboard_status: None,
        }
    }

    pub fn get_filtered_objects(&self) -> Vec<&ComObject> {
        filter_objects(&self.objects_list, &self.search_query)
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Check for background task completion
            if let Some(rx) = &self.inspection_receiver {
                match rx.try_recv() {
                    Ok(result) => {
                        match result {
                            Ok(details) => {
                                // Reset member selection when new object is loaded
                                if !details.members.is_empty() {
                                    self.member_list_state.select(Some(0));
                                } else {
                                    self.member_list_state.select(None);
                                }
                                self.selected_object = Some(details);
                            },
                            Err(e) => {
                                self.error_message = Some(format!("Error: {:#}", e));
                            }
                        }
                        self.inspection_receiver = None;
                    },
                    Err(TryRecvError::Empty) => {},
                    Err(TryRecvError::Disconnected) => {
                        self.error_message = Some("Inspection background task failed unexpectedly.".to_string());
                        self.inspection_receiver = None;
                    }
                }
            }

            terminal.draw(|f| ui_render(f, self))?;

            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press {
                        
                        // Clear transient clipboard status on any key press
                        if self.clipboard_status.is_some() {
                            self.clipboard_status = None;
                        }

                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            // Global Navigation
                            KeyCode::Esc => {
                                if self.app_mode == AppMode::Inspecting {
                                    self.exit_inspection();
                                } else if !self.search_query.is_empty() {
                                    self.search_query.clear();
                                    self.list_state.select(Some(0));
                                }
                            }
                            
                            // Mode Specific Handling
                            _ => match self.app_mode {
                                AppMode::Browsing => self.handle_browsing_input(key),
                                AppMode::Inspecting => self.handle_inspecting_input(key),
                                _ => {}
                            }
                        }
                    }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn handle_browsing_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                if !self.get_filtered_objects().is_empty() {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
            }
            KeyCode::Backspace => {
                let _ = self.search_query.pop();
                if !self.get_filtered_objects().is_empty() {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
            }
            KeyCode::Down => self.next_object(),
            KeyCode::Up => self.previous_object(),
            KeyCode::Enter => self.inspect_selected(),
            _ => {}
        }
    }

    fn handle_inspecting_input(&mut self, key: event::KeyEvent) {
        if let Some(details) = &self.selected_object {
            if details.members.is_empty() {
                return;
            }

            match key.code {
                KeyCode::Down => self.next_member(details.members.len()),
                KeyCode::Up => self.previous_member(details.members.len()),
                KeyCode::Char('c') => self.copy_selected_member_to_clipboard(),
                KeyCode::Char('C') => self.copy_all_members_to_clipboard(),
                _ => {}
            }
        }
    }

    fn next_object(&mut self) {
        let filtered = self.get_filtered_objects();
        if filtered.is_empty() { return; }
        
        let new_idx = match self.list_state.selected() {
            Some(i) => if i >= filtered.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(new_idx));
    }

    fn previous_object(&mut self) {
        let filtered = self.get_filtered_objects();
        if filtered.is_empty() { return; }

        let new_idx = match self.list_state.selected() {
            Some(i) => if i == 0 { filtered.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(new_idx));
    }

    fn next_member(&mut self, count: usize) {
        if count == 0 { return; }
        let new_idx = match self.member_list_state.selected() {
            Some(i) => if i >= count - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.member_list_state.select(Some(new_idx));
    }

    fn previous_member(&mut self, count: usize) {
        if count == 0 { return; }
        let new_idx = match self.member_list_state.selected() {
            Some(i) => if i == 0 { count - 1 } else { i - 1 },
            None => 0,
        };
        self.member_list_state.select(Some(new_idx));
    }

    fn inspect_selected(&mut self) {
        let clsid_opt = {
            let filtered = self.get_filtered_objects();
            self.list_state.selected()
                .and_then(|index| filtered.get(index))
                .map(|obj| obj.clsid.clone())
        };

        if let Some(clsid) = clsid_opt {
            self.selected_object = None;
            self.error_message = None;
            self.inspection_receiver = None;
            self.clipboard_status = None;
            self.member_list_state = ListState::default();
            
            self.app_mode = AppMode::Inspecting;

            let (tx, rx) = mpsc::channel();
            self.inspection_receiver = Some(rx);

            let clsid_clone = clsid.clone();
            
            thread::spawn(move || {
                let _com_guard = match com_interop::initialize_com() {
                    Ok(guard) => guard,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };

                let result = com_interop::get_type_info(&clsid_clone)
                    .context(format!("Failed to inspect COM object with CLSID {}", clsid_clone));
                
                let _ = tx.send(result);
            });
        }
    }

    fn exit_inspection(&mut self) {
        if self.app_mode == AppMode::Inspecting {
            self.app_mode = AppMode::Browsing;
            self.selected_object = None;
            self.error_message = None;
            self.inspection_receiver = None;
            self.clipboard_status = None;
            self.member_list_state = ListState::default();
        }
    }

    fn copy_selected_member_to_clipboard(&mut self) {
        if let Some(details) = &self.selected_object
            && let Some(idx) = self.member_list_state.selected()
                && let Some(member) = details.members.get(idx) {
                    let text_to_copy = match member {
                        Member::Method { name, signature, .. } => {
                            format!("{}{}", name, signature)
                        },
                        Member::Property { name, value_type, .. } => {
                            format!("{}: {}", name, value_type)
                        }
                    };

                    match Clipboard::new() {
                        Ok(mut clipboard) => {
                            if let Err(e) = clipboard.set_text(text_to_copy) {
                                self.clipboard_status = Some(format!("Clipboard error: {}", e));
                            } else {
                                self.clipboard_status = Some("Copied selection!".to_string());
                            }
                        },
                        Err(e) => {
                             self.clipboard_status = Some(format!("Clipboard init error: {}", e));
                        }
                    }
                }
    }

    fn copy_all_members_to_clipboard(&mut self) {
         if let Some(details) = &self.selected_object {
            let mut buffer = String::new();
            buffer.push_str(&format!("Type: {}\n", details.name));
            buffer.push_str(&format!("Description: {}\n", details.description));
            // We don't have CLSID directly in TypeDetails unless passed, omitting for now
            buffer.push('\n');
            
            for member in &details.members {
                match member {
                    Member::Method { name, signature, .. } => {
                        buffer.push_str(&format!("Method {}{}\n", name, signature));
                    },
                    Member::Property { name, value_type, access } => {
                         let access_str = match access {
                            AccessMode::Read => "Read",
                            AccessMode::Write => "Write",
                            AccessMode::ReadWrite => "Read/Write",
                        };
                        buffer.push_str(&format!("Property {}: {} [{}]\n", name, value_type, access_str));
                    }
                }
            }

            match Clipboard::new() {
                Ok(mut clipboard) => {
                    if let Err(e) = clipboard.set_text(buffer) {
                        self.clipboard_status = Some(format!("Clipboard error: {}", e));
                    } else {
                        self.clipboard_status = Some("Copied all members!".to_string());
                    }
                },
                Err(e) => {
                     self.clipboard_status = Some(format!("Clipboard init error: {}", e));
                }
            }
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
    let objects_list = &app.objects_list;
    let search_query = &app.search_query;
    let filtered_objects = filter_objects(objects_list, search_query);

    let items: Vec<ListItem> = filtered_objects
        .iter()
        .map(|obj| ListItem::new(format!("{} ({})", obj.name, obj.clsid)))
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
    
    f.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // Right Pane: Details or Inspection
    let right_pane_area = main_chunks[1];
    
    match app.app_mode {
        AppMode::Inspecting => {
            if let Some(err_msg) = &app.error_message {
                let p = Paragraph::new(vec![
                    Line::from(Span::styled("Error Inspecting Object:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(err_msg, Style::default().fg(Color::Red))),
                ]).block(Block::default().borders(Borders::ALL).title("Error"));
                f.render_widget(p, right_pane_area);
            } else if let Some(details) = &app.selected_object {
                // Split right pane into Metadata and Members
                let right_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(8), // Fixed height for metadata
                        Constraint::Min(0),    // Remaining for members
                    ])
                    .split(right_pane_area);

                // 1. Metadata Block
                let meta_text = vec![
                    Line::from(vec![Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&details.name)]),
                    Line::from(vec![Span::styled("Description: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&details.description)]),
                    Line::from(""),
                    Line::from(Span::styled("Copy: 'c' (Item) | 'Shift+C' (All)", Style::default().fg(Color::DarkGray))),
                ];
                
                let meta_block = Paragraph::new(meta_text)
                    .block(Block::default().borders(Borders::ALL).title("Object Details"))
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(meta_block, right_chunks[0]);

                // 2. Members List Block
                let members_list: Vec<ListItem> = details.members.iter().map(|m| {
                    match m {
                        Member::Method { name, signature, .. } => {
                            ListItem::new(Line::from(vec![
                                Span::styled("M ", Style::default().fg(Color::Cyan)), 
                                Span::raw(format!("{}{}", name, signature))
                            ]))
                        },
                        Member::Property { name, value_type, access } => {
                            let access_badge = match access {
                                AccessMode::Read => "R",
                                AccessMode::Write => "W",
                                AccessMode::ReadWrite => "RW",
                            };
                            ListItem::new(Line::from(vec![
                                Span::styled("P ", Style::default().fg(Color::Green)),
                                Span::styled(format!("[{}] ", access_badge), Style::default().fg(Color::DarkGray)),
                                Span::raw(format!("{}: {}", name, value_type))
                            ]))
                        }
                    }
                }).collect();

                let members_block = List::new(members_list)
                    .block(Block::default().borders(Borders::ALL).title("Members")
                    .style(Style::default().fg(Color::Yellow)))
                    .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
                    .highlight_symbol("> ");
                
                f.render_stateful_widget(members_block, right_chunks[1], &mut app.member_list_state);

            } else {
                let p = Paragraph::new("Loading...").block(Block::default().borders(Borders::ALL).title("Details"));
                f.render_widget(p, right_pane_area);
            }
        },
        _ => {
            // Browsing Mode Details
            let right_pane_block = Block::default()
                .borders(Borders::ALL)
                .title("Details");

            let details_text = if let Some(idx) = app.list_state.selected() {
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
            };

            let details = Paragraph::new(details_text)
                .block(right_pane_block)
                .wrap(ratatui::widgets::Wrap { trim: true });
            
            f.render_widget(details, right_pane_area);
        }
    };

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
        "".to_string()
    } else {
        format!(" | Search: '{}'", search_query)
    };

    let clipboard_msg = if let Some(status) = &app.clipboard_status {
        format!(" | {}", status)
    } else {
        "".to_string()
    };

    let status_text = format!(
        "Mode: {} | Obj: {} {}{} | <Enter>: Insp | <Esc>: Back | <c/C>: Copy", 
        mode_str,
        current_selection_name,
        search_status,
        clipboard_msg
    );
    let status = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[1]);
}
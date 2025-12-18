// src/app.rs
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Clear},
    Frame, Terminal,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};
use crate::scanner::ComObject;
use crate::error_handling::{Result, Context};
use crate::com_interop::{self, TypeDetails, Member, AccessMode};

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use arboard::Clipboard;
use std::collections::{VecDeque, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Scanning,
    Browsing,
    Inspecting,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeItem {
    Category { name: String, count: usize, expanded: bool },
    Object(ComObject), // Stores the ComObject directly
}

pub struct App {
    pub objects_list: Vec<ComObject>,
    pub search_query: String,
    pub list_state: ListState,
    pub app_mode: AppMode,
    pub should_quit: bool,
    
    // Categorization State
    pub expanded_categories: HashSet<String>,

    // State for Inspecting Mode
    pub selected_object: Option<TypeDetails>,
    pub error_message: Option<String>,
    pub inspection_receiver: Option<Receiver<Result<TypeDetails>>>,
    pub member_list_state: ListState,
    
    // Notification Queue
    pub notifications: VecDeque<Notification>,
    pub current_notification_start: Option<Instant>,
}

impl App {
    pub fn new(mut objects: Vec<ComObject>) -> Self {
        // Sort objects by name to ensure consistent initial order
        objects.sort_by(|a, b| a.name.cmp(&b.name));

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
            expanded_categories: HashSet::new(),
            selected_object: None,
            error_message: None,
            inspection_receiver: None,
            member_list_state: ListState::default(),
            notifications: VecDeque::new(),
            current_notification_start: None,
        }
    }

    pub fn show_notification(&mut self, message: String, duration_ms: u64) {
        self.notifications.push_back(Notification {
            message,
            duration: Duration::from_millis(duration_ms),
        });
    }

    fn tick_notifications(&mut self) {
        if let Some(notification) = self.notifications.front() {
            if self.current_notification_start.is_none() {
                self.current_notification_start = Some(Instant::now());
            }

            if let Some(start) = self.current_notification_start
                && start.elapsed() >= notification.duration {
                    self.notifications.pop_front();
                    self.current_notification_start = None;
                }
        }
    }

    /// Compiles the view items: Filters -> Groups -> Flattens based on expansion.
    /// Returns Vec<TreeItem> with ComObjects stored directly.
    pub fn get_view_items(&self) -> Vec<TreeItem> {
        let processed = crate::processor::process_objects(self.objects_list.clone(), &self.search_query);

        let mut items = Vec::new();
        // BTreeMap iterates keys alphabetically
        for (category, objs) in processed {
            let is_searching = !self.search_query.is_empty();
            let is_expanded = self.expanded_categories.contains(&category) || is_searching;
            
            items.push(TreeItem::Category { 
                name: category.clone(), 
                count: objs.len(), 
                expanded: is_expanded 
            });

            if is_expanded {
                for obj in objs {
                    items.push(TreeItem::Object(obj));
                }
            }
        }

        items
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Check for background task completion
            if let Some(rx) = &self.inspection_receiver {
                match rx.try_recv() {
                    Ok(result) => {
                        match result {
                            Ok(details) => {
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

            self.tick_notifications();

            // Calculate view items once per frame
            // Now returns Vec<TreeItem> which owns its data (indices), so no borrow of `self` persists
            let view_items = self.get_view_items();

            terminal.draw(|f| ui_render(f, self, &view_items))?;

            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Esc => {
                                if self.app_mode == AppMode::Inspecting {
                                    self.exit_inspection();
                                } else if !self.search_query.is_empty() {
                                    self.search_query.clear();
                                    self.list_state.select(Some(0));
                                }
                            }
                            
                            _ => match self.app_mode {
                                AppMode::Browsing => self.handle_browsing_input(key, &view_items),
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

    fn handle_browsing_input(&mut self, key: event::KeyEvent, view_items: &[TreeItem]) {
        match key.code {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                if !view_items.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            KeyCode::Backspace => {
                let _ = self.search_query.pop();
                if !view_items.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            KeyCode::Down => self.next_item(view_items.len()),
            KeyCode::Up => self.previous_item(view_items.len()),
            KeyCode::Enter => self.handle_enter_key(view_items),
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

    fn next_item(&mut self, count: usize) {
        if count == 0 { return; }
        let new_idx = match self.list_state.selected() {
            Some(i) => if i >= count - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(new_idx));
    }

    fn previous_item(&mut self, count: usize) {
        if count == 0 { return; }
        let new_idx = match self.list_state.selected() {
            Some(i) => if i == 0 { count - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(new_idx));
    }

    fn handle_enter_key(&mut self, view_items: &[TreeItem]) {
        if let Some(idx) = self.list_state.selected()
            && let Some(item) = view_items.get(idx) {
                match item {
                    TreeItem::Category { name, .. } => {
                        // Toggle expansion
                        if self.expanded_categories.contains(name) {
                            self.expanded_categories.remove(name);
                        } else {
                            self.expanded_categories.insert(name.clone());
                        }
                    },
                    TreeItem::Object(obj) => {
                        self.inspect_object(obj.clsid.clone());
                    }
                }
            }
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

    fn inspect_object(&mut self, clsid: String) {
        self.selected_object = None;
        self.error_message = None;
        self.inspection_receiver = None;
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
                .context(format!("Failed to inspect object {}. \nThis may be due to permissions or missing registration.", clsid_clone));
            
            let _ = tx.send(result);
        });
    }

    fn exit_inspection(&mut self) {
        if self.app_mode == AppMode::Inspecting {
            self.app_mode = AppMode::Browsing;
            self.selected_object = None;
            self.error_message = None;
            self.inspection_receiver = None;
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
                                self.show_notification(format!("Clipboard error: {}", e), 3000);
                            } else {
                                self.show_notification("Copied selection!".to_string(), 2000);
                            }
                        },
                        Err(e) => {
                             self.show_notification(format!("Clipboard init error: {}", e), 3000);
                        }
                    }
                }
    }

    fn copy_all_members_to_clipboard(&mut self) {
         if let Some(details) = &self.selected_object {
            let mut buffer = String::new();
            buffer.push_str(&format!("Type: {}\n", details.name));
            buffer.push_str(&format!("Description: {}\n", details.description));
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
                        self.show_notification(format!("Clipboard error: {}", e), 3000);
                    } else {
                        self.show_notification("Copied all members!".to_string(), 2000);
                    }
                },
                Err(e) => {
                     self.show_notification(format!("Clipboard init error: {}", e), 3000);
                }
            }
        }
    }
}

fn ui_render(f: &mut Frame, app: &mut App, view_items: &[TreeItem]) {
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

    // Left Pane: Object List (Tree View)
    let list_items: Vec<ListItem> = view_items.iter().map(|item| {
        match item {
            TreeItem::Category { name, count, expanded } => {
                let icon = if *expanded { "▼" } else { "▶" };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} {} ", icon, name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("({})", count), Style::default().fg(Color::DarkGray)),
                ]))
            },
            TreeItem::Object(obj) => {
                ListItem::new(Line::from(vec![
                    Span::raw("  "), // Indentation
                    Span::raw(&obj.name),
                    Span::styled(format!(" ({})", obj.clsid), Style::default().fg(Color::DarkGray)),
                ]))
            }
        }
    }).collect();

    let list_title = if app.search_query.is_empty() {
        "COM Objects".to_string()
    } else {
        format!("COM Objects (Filter: '{}')", app.search_query)
    };

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
        .highlight_symbol(" "); 
    
    f.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // Right Pane: Details or Inspection
    let right_pane_area = main_chunks[1];
    
    match app.app_mode {
        AppMode::Inspecting => {
            if let Some(err_msg) = &app.error_message {
                let p = Paragraph::new(vec![
                    Line::from(Span::styled("Error Inspecting Object:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(err_msg, Style::default().fg(Color::Red))),
                ])
                .block(Block::default().borders(Borders::ALL).title("Error"))
                .wrap(ratatui::widgets::Wrap { trim: true });
                
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
                if let Some(item) = view_items.get(idx) {
                    match item {
                        TreeItem::Category { name, count, .. } => vec![
                            Line::from(Span::styled("Category: ", Style::default().add_modifier(Modifier::BOLD))),
                            Line::from(name.as_str()),
                            Line::from(""),
                            Line::from(format!("Contains {} objects", count)),
                            Line::from(""),
                            Line::from(Span::styled("Hint: Press <Enter> to expand/collapse.", Style::default().fg(Color::Gray))),
                        ],
                        TreeItem::Object(obj) => {
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
                        }
                    }
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
         match view_items.get(idx) {
             Some(TreeItem::Category { name, .. }) => format!("Category: {}", name),
             Some(TreeItem::Object(obj)) => obj.name.clone(),
             None => "Unknown".to_string(),
         }
    } else {
        "None".to_string()
    };

    let mode_str = match app.app_mode {
        AppMode::Scanning => "SCANNING",
        AppMode::Browsing => "BROWSING",
        AppMode::Inspecting => "INSPECTING",
    };

    let search_status = if app.search_query.is_empty() {
        "".to_string()
    } else {
        format!(" | Search: '{}'", app.search_query)
    };

    let status_text = format!(
        "Mode: {} | Obj: {} {} | <Enter>: Expand/Insp | <Esc>: Back | <c/C>: Copy", 
        mode_str,
        current_selection_name,
        search_status
    );
    let status = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[1]);

    // Render Notification Modal Overlay
    if let Some(notification) = app.notifications.front() {
        let area = centered_rect_fixed_height(50, 3, f.area());
        
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Notification")
            .style(Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD));
            
        let paragraph = Paragraph::new(notification.message.as_str())
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .alignment(ratatui::layout::Alignment::Center);
            
        f.render_widget(Clear, area); // Clear area behind popup
        f.render_widget(paragraph, area);
    }
}

/// Helper function to create a centered rect of fixed height and percentage width
fn centered_rect_fixed_height(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
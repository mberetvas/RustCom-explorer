// src/error_handling.rs
use std::fmt;

#[derive(Debug)]
pub enum InspectError {
    Registry(String),
    Instantiation { message: String, hresult: Option<i32> },
    Parsing(String),
    Permission(String),
    Generic(String),
    Safety(String),
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InspectError::Registry(msg) => write!(f, "Registry Lookup Failed: {}", msg),
            InspectError::Instantiation { message, hresult } => {
                if let Some(hr) = hresult {
                    write!(f, "Instantiation Failed (Code: 0x{:08X})\nDetails: {}", hr, message)
                } else {
                    write!(f, "Instantiation Failed: {}", message)
                }
            },
            InspectError::Parsing(msg) => write!(f, "Type Parsing Failed: {}", msg),
            InspectError::Permission(msg) => write!(f, "Permission Denied: {}\nSuggestion: Try running the application as Administrator.", msg),
            InspectError::Generic(msg) => write!(f, "Error: {}", msg),
            InspectError::Safety(msg) => write!(f, "Safety Violation: {}", msg),
        }
    }
}

impl std::error::Error for InspectError {}

pub use anyhow::{Context, Result, Error};
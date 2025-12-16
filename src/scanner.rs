use crate::error_handling::{Result, Context};
use serde::{Serialize, Deserialize};

/// Represents a COM Object found in the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComObject {
    /// The Program ID (e.g., "Excel.Application")
    pub name: String,
    /// The CLSID GUID (e.g., "{00024500-0000-0000-C000-000000000046}")
    pub clsid: String,
    /// The description of the object (e.g., "Microsoft Excel Application")
    pub description: String,
}

/// Trait to abstract registry key operations for mocking.
pub trait RegistryKey {
    /// Opens a subkey.
    fn open_subkey(&self, name: &str) -> Result<Box<dyn RegistryKey>>;
    /// Returns a list of subkey names.
    fn get_sub_key_names(&self) -> Result<Vec<String>>;
    /// Gets the default string value of the key (name = "").
    fn get_value(&self, name: &str) -> Result<String>;
}

/// Trait to abstract the source of registry keys (specifically HKCR).
pub trait RegistryReader {
    fn get_classes_root(&self) -> Result<Box<dyn RegistryKey>>;
}

/// The main entry point for scanning COM objects.
///
/// On Windows, this uses the real registry.
/// On other platforms, it returns an empty list or error (here, empty for safety).
pub fn scan_com_objects() -> Result<Vec<ComObject>> {
    #[cfg(windows)]
    {
        let reader = windows_impl::WindowsRegistryReader;
        scan_com_objects_internal(&reader)
    }
    #[cfg(not(windows))]
    {
        // Graceful handling for non-Windows environments
        Ok(Vec::new())
    }
}

/// Internal scanning logic using the RegistryReader trait.
/// 
/// Iterates over HKEY_CLASSES_ROOT subkeys.
/// Filters for keys that have a "CLSID" subkey.
/// Extracts ProgID (key name), CLSID (default value of CLSID subkey),
/// and Description (default value of the key itself).
fn scan_com_objects_internal(reader: &impl RegistryReader) -> Result<Vec<ComObject>> {
    let root = reader.get_classes_root().context("Failed to open HKEY_CLASSES_ROOT")?;
    let mut objects = Vec::new();
    
    // We get all subkey names first.
    // In a real optimized scenario with millions of keys, we might prefer an iterator,
    // but Vec<String> is sufficient for standard HKCR sizes (~10-100k entries).
    let keys = root.get_sub_key_names().context("Failed to enumerate subkeys")?;

    for name in keys {
        // Filter: Check if "CLSID" subkey exists.
        // Logic: Open HKCR\<name>. Then try to open "CLSID".
        
        // Step 1: Open the potential ProgID key
        if let Ok(progid_key) = root.open_subkey(&name) {
            // Step 2: Check for "CLSID" subkey
            if let Ok(clsid_key) = progid_key.open_subkey("CLSID") {
                // Found a COM Object!
                
                // Step 3: Extract Metadata
                // CLSID is the default value of the ...\CLSID key
                let clsid_val = clsid_key.get_value("").unwrap_or_default();
                
                // Description is the default value of the ProgID key
                let description_val = progid_key.get_value("").unwrap_or_default();

                objects.push(ComObject {
                    name, // The ProgID is the key name itself
                    clsid: clsid_val,
                    description: description_val,
                });
            }
        }
    }

    Ok(objects)
}

// --- Windows Implementation ---

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use winreg::RegKey;
    use winreg::enums::*;

    pub struct WindowsRegistryReader;

    impl RegistryReader for WindowsRegistryReader {
        fn get_classes_root(&self) -> Result<Box<dyn RegistryKey>> {
            let key = RegKey::predef(HKEY_CLASSES_ROOT);
            Ok(Box::new(WindowsKey(key)))
        }
    }

    struct WindowsKey(RegKey);

    impl RegistryKey for WindowsKey {
        fn open_subkey(&self, name: &str) -> Result<Box<dyn RegistryKey>> {
            // open_subkey_with_flags is often safer/more precise, but open_subkey is standard read
            let key = self.0.open_subkey(name).map_err(crate::error_handling::Error::from)?;
            Ok(Box::new(WindowsKey(key)))
        }

        fn get_sub_key_names(&self) -> Result<Vec<String>> {
            let mut names = Vec::new();
            for name in self.0.enum_keys() {
                names.push(name.map_err(crate::error_handling::Error::from)?);
            }
            Ok(names)
        }

        fn get_value(&self, name: &str) -> Result<String> {
            self.0.get_value(name).map_err(crate::error_handling::Error::from)
        }
    }
}

// --- Tests (TDD) ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // Mock Structures
    #[derive(Clone)]
    struct MockKey {
        subkeys: Arc<Mutex<HashMap<String, MockKey>>>,
        values: Arc<Mutex<HashMap<String, String>>>,
    }

    impl MockKey {
        fn new() -> Self {
            Self {
                subkeys: Arc::new(Mutex::new(HashMap::new())),
                values: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn add_subkey(&self, name: &str, key: MockKey) {
            self.subkeys.lock().unwrap().insert(name.to_string(), key);
        }

        fn set_value(&self, name: &str, value: &str) {
            self.values.lock().unwrap().insert(name.to_string(), value.to_string());
        }
    }

    impl RegistryKey for MockKey {
        fn open_subkey(&self, name: &str) -> Result<Box<dyn RegistryKey>> {
            let map = self.subkeys.lock().unwrap();
            if let Some(key) = map.get(name) {
                Ok(Box::new(key.clone()))
            } else {
                Err(anyhow::anyhow!("Key not found"))
            }
        }

        fn get_sub_key_names(&self) -> Result<Vec<String>> {
            let map = self.subkeys.lock().unwrap();
            Ok(map.keys().cloned().collect())
        }

        fn get_value(&self, name: &str) -> Result<String> {
            let map = self.values.lock().unwrap();
            map.get(name).cloned().ok_or_else(|| anyhow::anyhow!("Value not found"))
        }
    }

    struct MockReader {
        root: MockKey,
    }

    impl RegistryReader for MockReader {
        fn get_classes_root(&self) -> Result<Box<dyn RegistryKey>> {
            Ok(Box::new(self.root.clone()))
        }
    }

    #[test]
    fn test_scan_com_objects_identifies_valid_entry() {
        // Setup Registry Structure:
        // HKCR
        //  |-- valid.progid  (Default: "My Description")
        //       |-- CLSID    (Default: "{123-456}")
        //  |-- invalid.entry
        //       |-- SomethingElse

        let root = MockKey::new();

        // 1. Valid COM Object
        let valid_key = MockKey::new();
        valid_key.set_value("", "My Description");
        
        let clsid_key = MockKey::new();
        clsid_key.set_value("", "{123-456}");
        
        valid_key.add_subkey("CLSID", clsid_key);
        root.add_subkey("valid.progid", valid_key);

        // 2. Invalid Entry (No CLSID subkey)
        let invalid_key = MockKey::new();
        invalid_key.add_subkey("NotCLSID", MockKey::new());
        root.add_subkey("invalid.entry", invalid_key);

        let reader = MockReader { root };

        // Act
        let results = scan_com_objects_internal(&reader).expect("Scan failed");

        // Assert
        assert_eq!(results.len(), 1);
        let obj = &results[0];
        assert_eq!(obj.name, "valid.progid");
        assert_eq!(obj.clsid, "{123-456}");
        assert_eq!(obj.description, "My Description");
    }

    #[test]
    fn test_scan_handles_missing_description_gracefully() {
        let root = MockKey::new();
        let progid = MockKey::new(); // No default value
        let clsid = MockKey::new();
        clsid.set_value("", "{GUID}");
        progid.add_subkey("CLSID", clsid);
        root.add_subkey("test.obj", progid);

        let reader = MockReader { root };
        let results = scan_com_objects_internal(&reader).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, ""); // Should be empty, not error
        assert_eq!(results[0].clsid, "{GUID}");
    }
}
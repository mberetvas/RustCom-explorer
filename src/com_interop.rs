use crate::error_handling::{Result, Context};
use windows::core::{GUID, HSTRING, PCWSTR};
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, CLSIDFromProgID, CLSIDFromString, COINIT_MULTITHREADED,
};

/// Initializes COM for the current thread with the Multi-threaded Apartment (MTA) model.
///
/// Returns a `ComGuard` that ensures `CoUninitialize` is called when it is dropped.
/// This should be called at the start of the application or thread (e.g., in `main`).
pub fn initialize_com() -> Result<ComGuard> {
    // SAFETY: CoInitializeEx is unsafe FFI. We pass None (NULL) for the reserved parameter
    // and COINIT_MULTITHREADED to set the concurrency model.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .context("Failed to initialize COM library")?;
    }
    Ok(ComGuard)
}

/// A guard that calls `CoUninitialize` on drop.
pub struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: CoUninitialize is unsafe FFI. It must be called to balance the successful
        // CoInitializeEx call. It is safe to call here as we are cleaning up the COM library
        // for this thread.
        unsafe {
            CoUninitialize();
        }
    }
}

/// Manually uninitializes COM.
///
/// Note: Prefer using `initialize_com` which returns a `ComGuard` for automatic cleanup.
/// This function is provided for manual control if needed.
pub fn uninitialize_com() {
    // SAFETY: CoUninitialize is unsafe FFI.
    unsafe {
        CoUninitialize();
    }
}

/// Converts a ProgID string (e.g., "Excel.Application") to a GUID.
pub fn progid_to_guid(progid: &str) -> Result<GUID> {
    let progid_h = HSTRING::from(progid);
    
    // SAFETY: CLSIDFromProgID requires a valid PCWSTR. HSTRING provides a null-terminated buffer.
    // We use PCWSTR::from_raw to convert the HSTRING pointer.
    let guid = unsafe {
        CLSIDFromProgID(PCWSTR::from_raw(progid_h.as_ptr()))
            .context(format!("Failed to resolve ProgID '{}' to GUID", progid))?
    };
    
    Ok(guid)
}

/// Converts a CLSID string (e.g., "{12345678-1234-1234-1234-1234567890AB}") to a GUID.
///
/// This function expects the standard registry format with braces.
pub fn clsid_string_to_guid(clsid_str: &str) -> Result<GUID> {
    let clsid_h = HSTRING::from(clsid_str);
    
    // SAFETY: CLSIDFromString requires a valid PCWSTR. HSTRING provides a null-terminated buffer.
    let guid = unsafe {
        CLSIDFromString(PCWSTR::from_raw(clsid_h.as_ptr()))
            .context(format!("Failed to parse CLSID string '{}'", clsid_str))?
    };
    
    Ok(guid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clsid_string_parsing() {
        // Initialize COM for the test thread (though CLSIDFromString is often standalone, it's safer).
        let _guard = initialize_com().unwrap();
        
        // Example: IUnknown GUID "{00000000-0000-0000-C000-000000000046}"
        let valid_clsid = "{00000000-0000-0000-C000-000000000046}";
        let result = clsid_string_to_guid(valid_clsid);
        assert!(result.is_ok());
        
        let guid = result.unwrap();
        // Check first part of data1 to verify it parsed something correct
        assert_eq!(guid.data1, 0);
    }

    #[test]
    fn test_invalid_clsid_string() {
         let _guard = initialize_com().unwrap();
         let invalid = "NotAGUID";
         assert!(clsid_string_to_guid(invalid).is_err());
    }
}
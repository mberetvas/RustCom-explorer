use crate::error_handling::{Result, Error};
use serde::{Serialize, Deserialize};
use std::ffi::c_void;
use windows::{
    core::{GUID, Interface, BSTR, PCWSTR},
    Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IIDFromString,
        CLSCTX_ALL, COINIT_MULTITHREADED,
        IDispatch, ITypeInfo, ITypeLib, TYPEATTR, FUNCDESC, VARDESC,
        INVOKE_FUNC, INVOKE_PROPERTYGET, INVOKE_PROPERTYPUT, INVOKE_PROPERTYPUTREF,
    },
    Win32::System::Ole::{
        LoadRegTypeLib,
    },
    Win32::System::Variant::{
        VARENUM, VT_BSTR, VT_I4, VT_UI4, VT_DISPATCH, VT_BOOL, VT_VARIANT, VT_UNKNOWN, VT_VOID,
        VT_I2, VT_R4, VT_R8, VT_CY, VT_DATE, VT_ERROR, VT_I1, VT_UI1, VT_UI2, VT_INT, VT_UINT,
        VT_HRESULT, VT_PTR, VT_SAFEARRAY, VT_USERDEFINED, VT_LPSTR, VT_LPWSTR,
    },
};
use winreg::{RegKey, enums::HKEY_CLASSES_ROOT};

/// RAII Guard for COM initialization
pub struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Initializes the COM library.
pub fn initialize_com() -> Result<ComGuard> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok();
    }
    Ok(ComGuard)
}

/// Details about a parsed COM Type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDetails {
    pub name: String,
    pub description: String,
    pub members: Vec<Member>,
}

/// Represents a member (Method or Property) of a COM object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "details")]
pub enum Member {
    Method {
        name: String,
        signature: String,
        return_type: String,
    },
    Property {
        name: String,
        value_type: String,
        access: AccessMode,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

pub fn get_type_info(clsid_str: &str) -> Result<TypeDetails> {
    let clsid = guid_from_str(clsid_str).unwrap_or(GUID::zeroed());
    
    // 1. Try Registry Strategy
    if let Ok(type_info) = load_type_info_from_registry(clsid_str) {
        return parse_type_info(&type_info, clsid_str);
    }

    // 2. Fallback: Dynamic Instantiation
    load_type_info_dynamic(&clsid)
}

fn guid_from_str(s: &str) -> Result<GUID> {
    // Ensure braces for IIDFromString
    let s_braced = if s.trim().starts_with('{') { s.to_string() } else { format!("{{{}}}", s) };
    let wide: Vec<u16> = s_braced.encode_utf16().chain(std::iter::once(0)).collect();
    
    unsafe {
        IIDFromString(PCWSTR::from_raw(wide.as_ptr()))
            .map_err(|e| Error::from(e))
    }
}

// --- Strategy 1: Registry Loading ---

fn load_type_info_from_registry(clsid_str: &str) -> Result<ITypeInfo> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let clsid_key = hkcr.open_subkey(format!("CLSID\\{}", clsid_str))?;
    
    let typelib_guid_str: String = clsid_key.open_subkey("TypeLib")?.get_value("")?;
    let typelib_guid = guid_from_str(&typelib_guid_str).map_err(|_| Error::msg("Invalid TypeLib GUID"))?;

    let version_str: String = clsid_key.open_subkey("Version")?.get_value("")?;
    let (major, minor) = parse_version(&version_str).unwrap_or((1, 0));

    unsafe {
        let type_lib: ITypeLib = LoadRegTypeLib(&typelib_guid, major, minor, 0)?;
        type_lib.GetTypeInfoOfGuid(&guid_from_str(clsid_str).unwrap_or_default())
            .or_else(|_| type_lib.GetTypeInfo(0))
    }.map_err(|e| Error::from(e))
}

fn parse_version(ver: &str) -> Option<(u16, u16)> {
    let parts: Vec<&str> = ver.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        Some((major, minor))
    } else if parts.len() == 1 {
        let major = parts[0].parse().ok()?;
        Some((major, 0))
    } else {
        None
    }
}

// --- Strategy 2: Dynamic Instantiation ---

fn load_type_info_dynamic(clsid: &GUID) -> Result<TypeDetails> {
    unsafe {
        let unknown: IDispatch = CoCreateInstance(clsid, None, CLSCTX_ALL)?;
        let type_info = unknown.GetTypeInfo(0, 0)?;
        parse_type_info(&type_info, &format!("{:?}", clsid))
    }
}

// --- Parsing Logic ---

fn parse_type_info(type_info: &ITypeInfo, default_name: &str) -> Result<TypeDetails> {
    let mut members = Vec::new();
    let attr = ScopedTypeAttr::new(type_info)?;
    let (name, doc) = get_documentation(type_info, -1).unwrap_or((default_name.to_string(), String::new()));

    unsafe {
        // Iterate Functions
        for i in 0..attr.0.cFuncs {
            if let Ok(func_desc) = ScopedFuncDesc::new(type_info, i as u32) {
                let desc = *func_desc.0;
                let (func_name, _) = get_documentation(type_info, desc.memid).unwrap_or(("Unknown".to_string(), String::new()));
                
                // GetNames expects a slice `&mut [BSTR]`
                let mut names = vec![BSTR::new(); 10]; 
                let mut c_names = 0;
                
                let _ = type_info.GetNames(
                    desc.memid, 
                    &mut names, // Pass slice directly
                    &mut c_names
                );
                
                let mut args = Vec::new();
                let param_count = desc.cParams as usize;
                let params_ptr = desc.lprgelemdescParam; 

                for p in 0..param_count {
                    let arg_name = if (p + 1) < c_names as usize {
                        names[p + 1].to_string()
                    } else {
                        format!("arg{}", p)
                    };
                    
                    let elem = *params_ptr.add(p);
                    // Extract .0 from VARENUM
                    let arg_type = vartype_to_string(elem.tdesc.vt.0);
                    args.push(format!("{}: {}", arg_name, arg_type));
                }

                // Extract .0 from VARENUM
                let return_type = vartype_to_string(desc.elemdescFunc.tdesc.vt.0);

                match desc.invkind {
                    INVOKE_FUNC => {
                        members.push(Member::Method {
                            name: func_name,
                            signature: format!("({}) -> {}", args.join(", "), return_type),
                            return_type,
                        });
                    },
                    INVOKE_PROPERTYGET | INVOKE_PROPERTYPUT | INVOKE_PROPERTYPUTREF => {
                        let access = if desc.invkind == INVOKE_PROPERTYGET { AccessMode::Read } else { AccessMode::Write };
                        let prop_type = if desc.invkind == INVOKE_PROPERTYGET {
                            return_type
                        } else {
                            if !args.is_empty() {
                                args.last().unwrap().split(": ").nth(1).unwrap_or("Variant").to_string()
                            } else {
                                "Variant".to_string()
                            }
                        };

                        members.push(Member::Property {
                            name: func_name,
                            value_type: prop_type,
                            access,
                        });
                    },
                    _ => {}
                }
            }
        }

        // Iterate Variables
        for i in 0..attr.0.cVars {
            if let Ok(var_desc) = ScopedVarDesc::new(type_info, i as u32) {
                let desc = *var_desc.0;
                let (var_name, _) = get_documentation(type_info, desc.memid).unwrap_or(("Unknown".to_string(), String::new()));
                // Extract .0 from VARENUM
                let var_type = vartype_to_string(desc.elemdescVar.tdesc.vt.0);
                
                members.push(Member::Property {
                    name: var_name,
                    value_type: var_type,
                    access: AccessMode::ReadWrite,
                });
            }
        }
    }

    Ok(TypeDetails {
        name,
        description: doc,
        members,
    })
}

fn get_documentation(type_info: &ITypeInfo, memid: i32) -> Result<(String, String)> {
    let mut name = BSTR::new();
    let mut doc_string = BSTR::new();
    unsafe {
        // Pass pointers wrapped in Some, and None for nulls
        type_info.GetDocumentation(
            memid, 
            Some(&mut name as *mut _), 
            Some(&mut doc_string as *mut _), 
            std::ptr::null_mut(), // pdwHelpContext is just *mut u32, not Option
            None // pbstrHelpFile is Option
        )?;
    }
    Ok((name.to_string(), doc_string.to_string()))
}

pub fn vartype_to_string(vt: u16) -> String {
    let base_type = vt & 0x0FFF; 
    let is_array = (vt & 0x2000) != 0;
    let is_byref = (vt & 0x4000) != 0;

    let type_name = match VARENUM(base_type) {
        VT_VOID => "Void",
        VT_I2 => "Short",
        VT_I4 => "Long",
        VT_R4 => "Single",
        VT_R8 => "Double",
        VT_CY => "Currency",
        VT_DATE => "Date",
        VT_BSTR => "String",
        VT_DISPATCH => "IDispatch",
        VT_ERROR => "Error",
        VT_BOOL => "Boolean",
        VT_VARIANT => "Variant",
        VT_UNKNOWN => "IUnknown",
        VT_I1 => "Byte",
        VT_UI1 => "Byte",
        VT_UI2 => "UShort",
        VT_UI4 => "ULong",
        VT_INT => "Int",
        VT_UINT => "UInt",
        VT_HRESULT => "HResult",
        VT_PTR => "Pointer",
        VT_SAFEARRAY => "SafeArray",
        VT_USERDEFINED => "UserDefined",
        VT_LPSTR => "String (LPSTR)",
        VT_LPWSTR => "String (LPWSTR)",
        _ => "Unknown",
    };

    let mut result = type_name.to_string();
    if is_array { result.push_str("[]"); }
    if is_byref { result.push_str("&"); }
    result
}

// --- RAII Wrappers ---

struct ScopedTypeAttr<'a>(&'a TYPEATTR, &'a ITypeInfo);
impl<'a> ScopedTypeAttr<'a> {
    fn new(info: &'a ITypeInfo) -> Result<Self> {
        unsafe {
            let ptr = info.GetTypeAttr()?;
            Ok(Self(&*ptr, info))
        }
    }
}
impl<'a> Drop for ScopedTypeAttr<'a> {
    fn drop(&mut self) {
        unsafe { self.1.ReleaseTypeAttr(self.0 as *const _ as *mut _) };
    }
}

struct ScopedFuncDesc<'a>(&'a FUNCDESC, &'a ITypeInfo);
impl<'a> ScopedFuncDesc<'a> {
    fn new(info: &'a ITypeInfo, index: u32) -> Result<Self> {
        unsafe {
            let ptr = info.GetFuncDesc(index)?;
            Ok(Self(&*ptr, info))
        }
    }
}
impl<'a> Drop for ScopedFuncDesc<'a> {
    fn drop(&mut self) {
        unsafe { self.1.ReleaseFuncDesc(self.0 as *const _ as *mut _) };
    }
}

struct ScopedVarDesc<'a>(&'a VARDESC, &'a ITypeInfo);
impl<'a> ScopedVarDesc<'a> {
    fn new(info: &'a ITypeInfo, index: u32) -> Result<Self> {
        unsafe {
            let ptr = info.GetVarDesc(index)?;
            Ok(Self(&*ptr, info))
        }
    }
}
impl<'a> Drop for ScopedVarDesc<'a> {
    fn drop(&mut self) {
        unsafe { self.1.ReleaseVarDesc(self.0 as *const _ as *mut _) };
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vartype_mapping() {
        assert_eq!(vartype_to_string(VT_BSTR.0 as u16), "String");
        assert_eq!(vartype_to_string(VT_I4.0 as u16), "Long");
        assert_eq!(vartype_to_string(VT_BOOL.0 as u16), "Boolean");
        assert_eq!(vartype_to_string((VT_I4.0 as u16) | 0x2000), "Long[]");
    }

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.2"), Some((1, 2)));
        assert_eq!(parse_version("5.0"), Some((5, 0)));
        assert_eq!(parse_version("1"), Some((1, 0)));
        assert_eq!(parse_version("invalid"), None);
    }
}
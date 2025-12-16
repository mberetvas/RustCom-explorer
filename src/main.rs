use comm_browser::scanner; 

fn main() {
    println!("Scanning for COM objects...");
    
    match scanner::scan_com_objects() {
        Ok(objects) => {
            if objects.is_empty() {
                println!("No COM objects found (or running on non-Windows OS).");
            } else {
                println!("Found {} COM objects.", objects.len());
                for (i, obj) in objects.iter().take(5).enumerate() {
                    println!("{}. {} ({}) - {}", i + 1, obj.name, obj.clsid, obj.description);
                }
                if objects.len() > 5 {
                    println!("... and {} more.", objects.len() - 5);
                }
            }
        },
        Err(e) => eprintln!("Error scanning registry: {:?}", e),
    }
}
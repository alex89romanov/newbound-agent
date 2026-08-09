// List contents of the BUILD_ME directory
let dir_path = "/home/nixos/BUILD_ME/";

if Path::new(dir_path).exists() {
    let entries = fs::read_dir(dir_path).expect("Failed to read directory");
    let mut result = String::new();
    result.push_str("Directory contents:\n");
    
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            let is_dir = path.is_dir();
            result.push_str(&format!("  {} ({}\n", file_name, if is_dir { "directory" } else { "file" }));
            
            // If it's a file, read its content
            if !is_dir {
                if let Ok(content) = fs::read_to_string(&path) {
                    result.push_str(&format!("    Content preview: {}\n", &content[..content.len().min(200)]));
                }
            }
        }
    }
    result
} else {
    format!("Directory {} does not exist", dir_path)
}
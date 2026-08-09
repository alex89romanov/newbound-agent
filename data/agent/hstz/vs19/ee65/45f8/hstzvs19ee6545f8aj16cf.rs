let dir_path = "/home/nixos/BUILD_ME/";
let mut result_obj = DataObject::new();

if Path::new(dir_path).exists() {
    let entries = fs::read_dir(dir_path).expect("Failed to read directory");
    let mut content = String::new();
    content.push_str("Directory contents:\n");
    
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let is_dir = path.is_dir();
            content.push_str(&format!("  {} ({})\n", file_name, if is_dir { "directory" } else { "file" }));
            
            if !is_dir {
                if let Ok(file_content) = fs::read_to_string(&path) {
                    let preview = if file_content.len() > 200 { &file_content[..200] } else { &file_content };
                    content.push_str(&format!("    Content preview: {}\n", preview));
                }
            }
        }
    }
    result_obj.put_string("status", "ok");
    result_obj.put_string("message", content);
} else {
    result_obj.put_string("status", "err");
    result_obj.put_string("message", format!("Directory {} does not exist", dir_path));
}

result_obj
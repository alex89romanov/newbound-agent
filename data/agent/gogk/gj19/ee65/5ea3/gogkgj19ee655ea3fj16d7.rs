let dir_path = "/home/nixos/BUILD_ME/";
let mut result = DataObject::new();

if !Path::new(dir_path).exists() {
    result.put_string("status", "error");
    result.put_string("message", &format!("Directory {} does not exist", dir_path));
    return result;
}

let mut files = DataArray::new();
let entries = match fs::read_dir(dir_path) {
    Ok(entries) => entries,
    Err(e) => {
        result.put_string("status", "error");
        result.put_string("message", &format!("Failed to read directory: {}", e));
        return result;
    }
};

for entry in entries {
    if let Ok(entry) = entry {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let is_dir = path.is_dir();
        
        let mut file_info = DataObject::new();
        file_info.put_string("name", &file_name);
        file_info.put_string("type", if is_dir { "directory" } else { "file" });
        
        if !is_dir {
            if let Ok(content) = fs::read_to_string(&path) {
                file_info.put_string("content", &content);
            } else {
                file_info.put_string("content", "[Unable to read file content]");
            }
        }
        
        files.push_object(file_info);
    }
}

result.put_string("status", "ok");
result.put_string("directory", dir_path);
result.put_array("files", files);
result
let dir_path = "/home/nixos/BUILD_ME/";
if Path::new(dir_path).exists() {
    "Directory exists".to_string()
} else {
    "Directory does not exist".to_string()
}
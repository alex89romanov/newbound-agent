// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::Transform;
pub mod describe_command;
pub mod list_tools;
pub mod control_query;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("innxiu19ebbb8efe6yfdf".to_string(), control_query::execute, "".to_string()));
    cmds.push(("sroyxx19ebde8708fk14aa".to_string(), list_tools::execute, "".to_string()));
    cmds.push(("ktoprh19ec10b7907k1b87".to_string(), describe_command::execute, "".to_string()));
}

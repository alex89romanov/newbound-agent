// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::Transform;
pub mod chat_llm;
pub mod tool_loop;
pub mod ask_llm;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("rjuoqv19e8fc5c83ft4".to_string(), ask_llm::execute, "".to_string()));
    cmds.push(("lnmvtl19edbeb72a7tc3a".to_string(), tool_loop::execute, "".to_string()));
    cmds.push(("ytohmk19f70b2c09ck7ce2".to_string(), chat_llm::execute, "".to_string()));
}

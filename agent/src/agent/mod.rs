// This file is auto-generated and managed by the flowlang build script.
use flowlang::rustcmd::Transform;
pub mod executive;
pub mod archivist;
pub mod scratch;
pub mod plugin;
pub mod llm;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    llm::cmdinit(cmds);
    plugin::cmdinit(cmds);
    scratch::cmdinit(cmds);
    archivist::cmdinit(cmds);
    executive::cmdinit(cmds);
}

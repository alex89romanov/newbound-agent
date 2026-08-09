use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::mcp::mcp::list_tools::list_tools;
use flowlang::mcp::mcp::invoke::invoke;
use std::collections::HashSet;
use ndata::Data::DString;
use crate::agent::llm::tool_loop::tool_loop;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["message", "context"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: String = o.get_string("message");
        let arg_1: DataObject = o.get_object("context");
        control_query(arg_0, arg_1)
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_string("a", &ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn control_query(message: String, context: DataObject) -> String {
// Tool Use Pattern Deprecated!
// We are moving to agent-llm-tool_loop

/* Filters a list of tools based on a list of enabled tool names.
fn filter_enabled_tools(all_tools: &DataObject, context: &DataObject) -> DataObject {
    let enabled_names: HashSet<String> = context
        .try_get_array("enabled_tools")
        .ok()
        .map_or_else(HashSet::new, |arr| {
            (0..arr.len())
                .filter_map(|i| arr.try_get_string(i).ok())
                .collect()
        });

    let mut result = DataObject::new();
    let mut filtered_tools = DataArray::new();

    if enabled_names.is_empty() {
        result.put_array("tools", filtered_tools);
        return result;
    }

    if let Ok(all_tools_array) = all_tools.try_get_array("tools") {
        for i in 0..all_tools_array.len() {
            if let Ok(tool_obj) = all_tools_array.try_get_object(i) {
                if let Ok(name) = tool_obj.try_get_string("name") {
                    if enabled_names.contains(&name) {
                        filtered_tools.push_object(tool_obj);
                    }
                }
            }
        }
    }

    result.put_array("tools", filtered_tools);
    result
}
*/

/// Maps Newbound's internal type names to their Rust equivalents.
fn lookup_rust_api_data_type(meta_type: &str) -> &str {
    match meta_type {
        "FLAT" | "JSONObject" => "DataObject", "JSONArray" => "DataArray",
        "InputStream" => "DataBytes", "float" => "f64", "Integer" => "i64",
        "Boolean" => "bool", "Any" => "Data", "NULL" => "DNull",
        _ => "String",
    }
}

// --- Main Chat Logic ---
/*
let all_mcp_tools = list_tools();
let enabled_tools = filter_enabled_tools(&all_mcp_tools, &context);
let tools_json = enabled_tools.to_string();
let tool_instructions = if enabled_tools.get_array("tools").len() > 0 {
    format!(r#"
You have access to a set of tools. If you need to use a tool, your response MUST contain a code block labled "tool.json" containing a JSON code block containing only the tool call.

Example Response:
<think>The user wants to list files. I will use the 'fs-list' tool.</think>
```tool.json
{{
  "tool": "fs-list",
  "args": {{ "path": "./" }}
}}
```

Here are the available tools:
{}
"#, tools_json)
} else { String::new() };
*/

// The base system prompt from the file
//let mut base_system_prompt = include_str!("/newbound/runtime/agent/prompt2.txt").to_string();
//base_system_prompt += "\n\n";
//base_system_prompt += &tool_instructions; // Add tool instructions to the system prompt

let lib = context.get_string("lib");
let ctl = context.get_string("ctl");
let mut context_prompt = format!("\n\nWe are editing the {} control in the {} library.", ctl, lib);

// Iterate through the code sections (html, css, js, cmd, behavior) provided in the context.
for k in ["html", "css", "js", "cmd", "behaviorCode"] {
    if let Ok(v) = context.try_get_string(k) {
        let v = v.trim();
        if !v.is_empty() {
            let mut chunk = "\n\n```".to_string();
            let mut signature = String::new();

            // If it's a command, construct the full signature from the context metadata.
            if k == "cmd" {
                let cmd = context.get_string("cmdname");
                let lang = context.get_string("cmdlang");
                chunk += &format!("{}:{}:{}.{}", lib, ctl, &cmd, &lang);

                if let Ok(params_arr) = context.try_get_array("cmdparams") {
                    let mut params_vec: Vec<String> = Vec::new();
                    for i in 0..params_arr.len() {
                        if let Ok(param_obj) = params_arr.try_get_object(i) {
                            let name = param_obj.get_string("name");
                            if lang == "rs" {
                                let meta_type = param_obj.get_string("type");
                                let rust_type = lookup_rust_api_data_type(&meta_type);
                                params_vec.push(format!("{}: {}", name, rust_type));
                            } else if lang == "py" {
                                params_vec.push(name);
                            }
                        }
                    }
                    let params_str = params_vec.join(", ");
                    if lang == "rs" {
                        let ret_meta_type = context.get_string("cmdreturn");
                        let rust_ret_type = lookup_rust_api_data_type(&ret_meta_type);
                        signature = format!("// fn {}({}) -> {}\n", cmd, params_str, rust_ret_type);
                    } else if lang == "py" {
                        signature = format!("# def {}({}):\n", cmd, params_str);
                    }
                }
            } else if k == "behaviorCode" {
                if let Ok(name) = context.try_get_string("behaviorName") {
                    chunk += &format!("behavior:{}", name);
                    // Create a simple JS function signature, as params are not yet passed from the frontend.
                    signature = format!("// function {} (/...params.../) {{\n", name);
                } else {
                    // If we have code but no name, something is wrong; skip this context block.
                    continue;
                }
            } else {
                chunk += &format!("{}:{}.{}", lib, ctl, k);
            }

            // Strip any pre-existing signature from the provided code to avoid duplication.
            let mut code_body = v.to_string();
            if k == "cmd" && !signature.is_empty() {
                if let Some(first_line) = code_body.lines().next() {
                    let lang = context.get_string("cmdlang");
                    let first_line_trimmed = first_line.trim();
                    let is_rust_sig = lang == "rs" && first_line_trimmed.starts_with("// fn ");
                    let is_py_sig = lang == "py" && first_line_trimmed.starts_with("# def ");

                    if is_rust_sig || is_py_sig {
                        code_body = code_body.lines().skip(1).collect::<Vec<&str>>().join("\n");
                    }
                }
            }

            // Assemble the final code block for the context_prompt.
            chunk += "\n";
            chunk += &signature;
            chunk += &code_body;
            chunk += "\n```\n";
            context_prompt += &chunk.replace('\u{00A0}', " ");
        }
    }
}

// Build conversation history string from context
let mut history_str = String::new();
if let Ok(history_arr) = context.try_get_array("chat_history") {
    for i in 0..history_arr.len() {
        if let Ok(msg_obj) = history_arr.try_get_object(i) {
            if let (Ok(role), Ok(content)) = (msg_obj.try_get_string("role"), msg_obj.try_get_string("content")) {
                let formatted_role = if role == "user" { "User" } else { "Assistant" };
                history_str += &format!("\n\n{}: {}", formatted_role, content);
            }
        }
    }
}

// Combine context and history into the main conversation prompt
let mut conversation_prompt = context_prompt + &history_str + "\n\nUser: " + &message;

const MAX_ITERATIONS: usize = 5;

//let thewholeenchilada = base_system_prompt.clone() + "\n\n" + &conversation_prompt  + "\n\nAssistant:\n-----------------------------------------------------";
//println!("FULL PROMPT TO LLM:\n{}", thewholeenchilada);
//std::fs::write("/home/mraiser/Desktop/last_call.txt", &thewholeenchilada).ok();
println!("FULL PROMPT TO LLM:\n{}", conversation_prompt);
std::fs::write("/home/mraiser/Desktop/last_call.txt", &conversation_prompt).ok();

// 4. Main conversation loop
for i in 0..MAX_ITERATIONS {
    // The prompt for the LLM now includes conversation history and user message, but not the system prompt.
    let full_prompt_to_llm = conversation_prompt.clone() + "\n\nAssistant:";
    println!("LOOP {} - PROMPT TO LLM:\n{}", i, full_prompt_to_llm);

    //let llm_response = ask_llm(full_prompt_to_llm, DString(base_system_prompt.clone()));
    let llm_response = tool_loop(full_prompt_to_llm).get_string("msg");
    println!("LOOP {} - RAW RESPONSE:\n{}", i, llm_response);

    let mut tool_call_found = false;
    if let Some(code_block_start) = llm_response.find("```tool.json") {
        let content_start = code_block_start + "```tool.json\n".len();
        if let Some(search_area_for_end) = llm_response.get(content_start..) {
            if let Some(relative_end) = search_area_for_end.find("```") {
                let content_end = content_start + relative_end;
                let json_str = &llm_response[content_start..content_end].trim();

                if let Ok(tool_call) = DataObject::try_from_string(json_str) {
                    if tool_call.has("tool") && tool_call.has("args") {
                        
                        // Create a new DataObject that matches the format expected by invoke.
                        let mut invoke_payload = DataObject::new();
                        
                        // invoke expects "name", not "tool"
                        if let Ok(tool_name) = tool_call.try_get_string("tool") {
                            invoke_payload.put_string("name", &tool_name);
                        }
                        
                        // invoke expects "arguments", not "args"
                        if let Ok(tool_args) = tool_call.try_get_object("args") {
                            invoke_payload.put_object("arguments", tool_args);
                        }
                        
                        println!("Detected and transformed tool call: {}", invoke_payload.to_string());
                        
                        let tool_result = invoke(invoke_payload);
                        let tool_result_str = tool_result.to_string();
                        println!("Tool result: {}", tool_result_str);

                        conversation_prompt += &format!("\n\nAssistant: {}\n\nTool Output: {}", llm_response, tool_result_str);
                        tool_call_found = true;
                    }
                }
            }
        }
    }

    if tool_call_found {
        continue;
    } else {
        std::fs::write("/home/mraiser/Desktop/last_response.txt", &llm_response).ok();
        return llm_response;
    }
}

return "Error: Reached maximum number of tool use iterations.".to_string();
}

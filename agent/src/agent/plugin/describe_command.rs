use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use crate::agent::llm::ask_llm::ask_llm;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["command_name", "lang", "returntype", "groups", "params", "imports", "code", "current_description"] {
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
        let arg_0: String = o.get_string("command_name");
        let arg_1: String = o.get_string("lang");
        let arg_2: String = o.get_string("returntype");
        let arg_3: String = o.get_string("groups");
        let arg_4: DataArray = o.get_array("params");
        let arg_5: String = o.get_string("imports");
        let arg_6: String = o.get_string("code");
        let arg_7: String = o.get_string("current_description");
        describe_command(arg_0, arg_1, arg_2, arg_3, arg_4, arg_5, arg_6, arg_7)
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

pub fn describe_command(command_name: String, lang: String, returntype: String, groups: String, params: DataArray, imports: String, code: String, current_description: String) -> String {
let mut prompt = String::new();

prompt.push_str("You are an AI assistant tasked with generating a concise and informative description for a backend command in the Newbound IDE.");
prompt.push_str("\n\n--- COMMAND DETAILS ---");
prompt.push_str(&format!("\nCommand Name: \"{}\"", command_name));
prompt.push_str(&format!("\nLanguage: \"{}\"", lang));
prompt.push_str(&format!("\nReturn Type: \"{}\"", returntype));
if !groups.is_empty() {
  prompt.push_str(&format!("\nGroups: \"{}\"", groups));
}

if params.len() > 0 {
  prompt.push_str("\n\nParameters:");
  // Iterate through the DataArray of parameter DataObjects
  for i in 0..params.len() {
    let p_obj = params.get_object(i); // Get the DataObject for the current parameter
    let p_name = p_obj.get_string("name");
    let p_type = p_obj.get_string("type");
    // Check if "desc" exists before attempting to get it to avoid panic
    let p_desc = if p_obj.has("desc") {
      p_obj.get_string("desc")
    } else {
      String::new()
    };

    prompt.push_str(&format!("\n  - Name: \"{}\", Type: \"{}\"", p_name, p_type));
    if !p_desc.is_empty() {
      prompt.push_str(&format!(", Current Description: \"{}\"", p_desc));
    }
  }
} else {
  prompt.push_str("\n\nParameters: None");
}

if !imports.is_empty() {
  prompt.push_str(&format!("\n\nImports:\n```{}\n{}\n```", lang, imports));
}

if !code.is_empty() {
  prompt.push_str(&format!("\n\nCode Snippet:\n```{}\n{}\n```", lang, code));
}

prompt.push_str("\n\n--- INSTRUCTIONS ---");
prompt.push_str("\nBased on the above command details, generate a new, concise, and professional description for this command.");
prompt.push_str("\nFocus on what the command *does*, its *purpose*, and its *inputs/outputs*.");
prompt.push_str("\nKeep it to 1-3 sentences.");

if !current_description.is_empty() {
  prompt.push_str(&format!("\n\nExisting Description (improve if necessary): \"{}\"", current_description));
} else {
  prompt.push_str("\n\nNo existing description provided. Generate a new one.");
}

prompt.push_str("\n\nNew Command Description:");
ask_llm(prompt, ndata::Data::DNull)
}

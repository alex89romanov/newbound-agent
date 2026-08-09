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
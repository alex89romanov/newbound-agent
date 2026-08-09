use flowlang::datastore::DataStore;
use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;

let api = crate::api::new();
let store = DataStore::new();
let mut results = DataArray::new();

// Retrieve all libraries available in the environment
let libs_array = api.app.app.libs();

for i in 0..libs_array.len() {
    let current_lib = libs_array.get_string(i);
    
    // Scope to specific lib if provided
    if !lib.is_empty() && current_lib != lib {
        continue;
    }

    if store.exists(&current_lib, "controls") {
        let ctls_obj = store.get_data(&current_lib, "controls").get_object("data");
        if ctls_obj.has("list") {
            let ctls_list = ctls_obj.get_array("list");
            
            for j in 0..ctls_list.len() {
                let ctl_item = ctls_list.get_object(j);
                let current_ctl = ctl_item.get_string("name");
                let ctl_id = ctl_item.get_string("id");

                // Scope to specific ctl if provided
                if !ctl.is_empty() && current_ctl != ctl {
                    continue;
                }

                if store.exists(&current_lib, &ctl_id) {
                    let ctl_doc = store.get_data(&current_lib, &ctl_id).get_object("data");
                    
                    if ctl_doc.has("cmd") {
                        let cmds_list = ctl_doc.get_array("cmd");
                        
                        for k in 0..cmds_list.len() {
                            let cmd_item = cmds_list.get_object(k);
                            let current_cmd = cmd_item.get_string("name");
                            let cmd_id = cmd_item.get_string("id");

                            if store.exists(&current_lib, &cmd_id) {
                                let cmd_data = store.get_data(&current_lib, &cmd_id).get_object("data");
                                let lang = if cmd_data.has("type") { cmd_data.get_string("type") } else { "rust".to_string() };
                                
                                if cmd_data.has(&lang) {
                                    let impl_id = cmd_data.get_string(&lang);
                                    
                                    if store.exists(&current_lib, &impl_id) {
                                        let impl_data = store.get_data(&current_lib, &impl_id).get_object("data");
                                        let ext = if lang == "rust" { "rs" } else { &lang };
                                        
                                        if impl_data.has(&ext) {
                                            let code_body = impl_data.get_string(&ext);
                                            
                                            if code_body.contains(&query) {
                                                let mut match_obj = DataObject::new();
                                                match_obj.put_string("lib", &current_lib);
                                                match_obj.put_string("ctl", &current_ctl);
                                                match_obj.put_string("cmd", &current_cmd);
                                                results.push_object(match_obj);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

results
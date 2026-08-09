#![allow(non_camel_case_types, unused_variables)]
use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::data::Data;

pub struct flow_case {}
pub struct flow_checkicon {}
pub struct flow_conditionalicon {}
pub struct flow_editor {}
pub struct flow_failicon {}
pub struct flow_inputbar {}
pub struct flow_listicon {}
pub struct flow_loopicon {}
pub struct flow_node {}
pub struct flow_node_editor {}
pub struct flow_operation {}
pub struct flow_operation_editor {}
pub struct flow_xicon {}
pub struct flow_interpreter {}
pub struct app_api {}
pub struct app_app {}
pub struct app_appcard {}
pub struct app_appinfo {}
pub struct app_dial {}
pub struct app_list {}
pub struct app_list_item {}
pub struct app_login {}
pub struct app_scenegraph {}
pub struct app_select {}
pub struct app_service {}
pub struct app_shape {}
pub struct app_ui {}
pub struct app_ui_reference {}
pub struct app_util {}
pub struct app_player {}
pub struct app_sceneplayer {}
pub struct app_sceneexpr {}
pub struct app_scenetokens {}
pub struct app_scenedoc {}
pub struct app_sceneproject {}
pub struct app_scenerun {}
pub struct app_forcelayout {}
pub struct app_store {}
pub struct app_nb {}
pub struct app_tokens {}
pub struct app_webgl {}
pub struct app_loader {}
pub struct app_bridge {}
pub struct app_modules {}
pub struct kb_platform_api {}
pub struct kb_workflow {}
pub struct kb_frontend {}
pub struct kb_m2026_07 {}
pub struct agent_agent {}
pub struct agent_llm {}
pub struct agent_plugin {}
pub struct agent_scratch {}
pub struct agent_agentloop {}
pub struct agent_agentprompt {}
pub struct agent_agentmodules {}
pub struct agent_memory {}
pub struct agent_archivist {}
pub struct agent_chat {}
pub struct dev_dev {}
pub struct dev_editcommand {}
pub struct dev_editcontrol {}
pub struct dev_github {}
pub struct dev_libsettings {}
pub struct dev_plugins {}
pub struct dev_workbench {}
pub struct dev_sceneeditor {}
pub struct dev_floweditor {}
pub struct dev_floweditor3d {}
pub struct dev_editor {}
pub struct dev_preview {}
pub struct dev_shelf {}
pub struct dev_card {}
pub struct dev_jump {}
pub struct dev_frame {}
pub struct dev_toast {}
pub struct dev_session {}
pub struct dev_flowdoc {}
pub struct dev_flowproject {}
pub struct dev_flowprims {}
pub struct dev_flowlayout {}
pub struct dev_facets {}
pub struct dev_chatctx {}
pub struct dev_devmodules {}
pub struct dev_code {}
pub struct dev_prompts {}
pub struct peer_headsup {}
pub struct peer_peer {}
pub struct peer_peer_model {}
pub struct peer_reboot {}
pub struct peer_service {}
pub struct peer_peer_select {}
pub struct scratch_scratch {}
pub struct security_security {}

pub struct flow {
    pub case: flow_case,
    pub checkicon: flow_checkicon,
    pub conditionalicon: flow_conditionalicon,
    pub editor: flow_editor,
    pub failicon: flow_failicon,
    pub inputbar: flow_inputbar,
    pub listicon: flow_listicon,
    pub loopicon: flow_loopicon,
    pub node: flow_node,
    pub node_editor: flow_node_editor,
    pub operation: flow_operation,
    pub operation_editor: flow_operation_editor,
    pub xicon: flow_xicon,
    pub interpreter: flow_interpreter,
}
pub struct app {
    pub api: app_api,
    pub app: app_app,
    pub appcard: app_appcard,
    pub appinfo: app_appinfo,
    pub dial: app_dial,
    pub list: app_list,
    pub list_item: app_list_item,
    pub login: app_login,
    pub scenegraph: app_scenegraph,
    pub select: app_select,
    pub service: app_service,
    pub shape: app_shape,
    pub ui: app_ui,
    pub ui_reference: app_ui_reference,
    pub util: app_util,
    pub player: app_player,
    pub sceneplayer: app_sceneplayer,
    pub sceneexpr: app_sceneexpr,
    pub scenetokens: app_scenetokens,
    pub scenedoc: app_scenedoc,
    pub sceneproject: app_sceneproject,
    pub scenerun: app_scenerun,
    pub forcelayout: app_forcelayout,
    pub store: app_store,
    pub nb: app_nb,
    pub tokens: app_tokens,
    pub webgl: app_webgl,
    pub loader: app_loader,
    pub bridge: app_bridge,
    pub modules: app_modules,
}
pub struct kb {
    pub platform_api: kb_platform_api,
    pub workflow: kb_workflow,
    pub frontend: kb_frontend,
    pub m2026_07: kb_m2026_07,
}
pub struct agent {
    pub agent: agent_agent,
    pub llm: agent_llm,
    pub plugin: agent_plugin,
    pub scratch: agent_scratch,
    pub agentloop: agent_agentloop,
    pub agentprompt: agent_agentprompt,
    pub agentmodules: agent_agentmodules,
    pub memory: agent_memory,
    pub archivist: agent_archivist,
    pub chat: agent_chat,
}
pub struct dev {
    pub dev: dev_dev,
    pub editcommand: dev_editcommand,
    pub editcontrol: dev_editcontrol,
    pub github: dev_github,
    pub libsettings: dev_libsettings,
    pub plugins: dev_plugins,
    pub workbench: dev_workbench,
    pub sceneeditor: dev_sceneeditor,
    pub floweditor: dev_floweditor,
    pub floweditor3d: dev_floweditor3d,
    pub editor: dev_editor,
    pub preview: dev_preview,
    pub shelf: dev_shelf,
    pub card: dev_card,
    pub jump: dev_jump,
    pub frame: dev_frame,
    pub toast: dev_toast,
    pub session: dev_session,
    pub flowdoc: dev_flowdoc,
    pub flowproject: dev_flowproject,
    pub flowprims: dev_flowprims,
    pub flowlayout: dev_flowlayout,
    pub facets: dev_facets,
    pub chatctx: dev_chatctx,
    pub devmodules: dev_devmodules,
    pub code: dev_code,
    pub prompts: dev_prompts,
}
pub struct peer {
    pub headsup: peer_headsup,
    pub peer: peer_peer,
    pub peer_model: peer_peer_model,
    pub reboot: peer_reboot,
    pub service: peer_service,
    pub peer_select: peer_peer_select,
}
pub struct scratch {
    pub scratch: scratch_scratch,
}
pub struct security {
    pub security: security_security,
}

pub struct api {
    pub flow: flow,
    pub app: app,
    pub kb: kb,
    pub agent: agent,
    pub dev: dev,
    pub peer: peer,
    pub scratch: scratch,
    pub security: security,
}
pub const fn new() -> api {
    api {
        flow: flow {
            case: flow_case {},
            checkicon: flow_checkicon {},
            conditionalicon: flow_conditionalicon {},
            editor: flow_editor {},
            failicon: flow_failicon {},
            inputbar: flow_inputbar {},
            listicon: flow_listicon {},
            loopicon: flow_loopicon {},
            node: flow_node {},
            node_editor: flow_node_editor {},
            operation: flow_operation {},
            operation_editor: flow_operation_editor {},
            xicon: flow_xicon {},
            interpreter: flow_interpreter {},
        },
        app: app {
            api: app_api {},
            app: app_app {},
            appcard: app_appcard {},
            appinfo: app_appinfo {},
            dial: app_dial {},
            list: app_list {},
            list_item: app_list_item {},
            login: app_login {},
            scenegraph: app_scenegraph {},
            select: app_select {},
            service: app_service {},
            shape: app_shape {},
            ui: app_ui {},
            ui_reference: app_ui_reference {},
            util: app_util {},
            player: app_player {},
            sceneplayer: app_sceneplayer {},
            sceneexpr: app_sceneexpr {},
            scenetokens: app_scenetokens {},
            scenedoc: app_scenedoc {},
            sceneproject: app_sceneproject {},
            scenerun: app_scenerun {},
            forcelayout: app_forcelayout {},
            store: app_store {},
            nb: app_nb {},
            tokens: app_tokens {},
            webgl: app_webgl {},
            loader: app_loader {},
            bridge: app_bridge {},
            modules: app_modules {},
        },
        kb: kb {
            platform_api: kb_platform_api {},
            workflow: kb_workflow {},
            frontend: kb_frontend {},
            m2026_07: kb_m2026_07 {},
        },
        agent: agent {
            agent: agent_agent {},
            llm: agent_llm {},
            plugin: agent_plugin {},
            scratch: agent_scratch {},
            agentloop: agent_agentloop {},
            agentprompt: agent_agentprompt {},
            agentmodules: agent_agentmodules {},
            memory: agent_memory {},
            archivist: agent_archivist {},
            chat: agent_chat {},
        },
        dev: dev {
            dev: dev_dev {},
            editcommand: dev_editcommand {},
            editcontrol: dev_editcontrol {},
            github: dev_github {},
            libsettings: dev_libsettings {},
            plugins: dev_plugins {},
            workbench: dev_workbench {},
            sceneeditor: dev_sceneeditor {},
            floweditor: dev_floweditor {},
            floweditor3d: dev_floweditor3d {},
            editor: dev_editor {},
            preview: dev_preview {},
            shelf: dev_shelf {},
            card: dev_card {},
            jump: dev_jump {},
            frame: dev_frame {},
            toast: dev_toast {},
            session: dev_session {},
            flowdoc: dev_flowdoc {},
            flowproject: dev_flowproject {},
            flowprims: dev_flowprims {},
            flowlayout: dev_flowlayout {},
            facets: dev_facets {},
            chatctx: dev_chatctx {},
            devmodules: dev_devmodules {},
            code: dev_code {},
            prompts: dev_prompts {},
        },
        peer: peer {
            headsup: peer_headsup {},
            peer: peer_peer {},
            peer_model: peer_peer_model {},
            reboot: peer_reboot {},
            service: peer_service {},
            peer_select: peer_peer_select {},
        },
        scratch: scratch {
            scratch: scratch_scratch {},
        },
        security: security {
            security: security_security {},
        },
    }
}

impl app_app {
    pub fn apps (&self) -> DataArray {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("ynjjnl182f0c30c2ej26bb").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn asset (&self, nn_path: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("nn_path", &nn_path);
        flowlang::rustcmd::RustCmd::new("hxusrn182ebab0fc8o1102").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn assets (&self, lib: String) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("uirppm183059f5a37z1b0c").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn delete (&self, lib: String, id: String, nn_sessionid: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("id", &id);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("mhnrjq18347bcd5f7t27").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn deletelib (&self, lib: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("hkgorn1834268eb07k1406").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn deviceid (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("jypyqw1836795f8fbn2").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn eventoff (&self, id: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        flowlang::rustcmd::RustCmd::new("xrysgt18350cb35cet3").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn eventon (&self, id: String, app: String, event: String, cmdlib: String, cmdid: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        d.put_string("app", &app);
        d.put_string("event", &event);
        d.put_string("cmdlib", &cmdlib);
        d.put_string("cmdid", &cmdid);
        flowlang::rustcmd::RustCmd::new("wlnoru18350ecc36cr4").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn events (&self, app: String) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("app", &app);
        flowlang::rustcmd::RustCmd::new("spumvi1834c2cf1e6t2").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn exec (&self, lib: String, id: String, args: DataObject, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("id", &id);
        d.put_object("args", args);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("thoxjp182ee8eaebdt225").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn jsapi (&self, nn_path: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("nn_path", &nn_path);
        flowlang::rustcmd::RustCmd::new("zmzwjn182ee9c7f0ar314").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn libs (&self) -> DataArray {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("vtnluk1834262fb3fl137e").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn login (&self, user: String, pass: String, nn_sessionid: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("user", &user);
        d.put_string("pass", &pass);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("ztizvj182ee99186cp2d2").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn newlib (&self, lib: String, readers: DataArray, writers: DataArray) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_array("readers", readers);
        d.put_array("writers", writers);
        flowlang::rustcmd::RustCmd::new("stskpj183421d8115xd3f").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn read (&self, lib: String, id: String, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("id", &id);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("nyzimq182eabf7339p7c5").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn remembersession (&self, nn_session: DataObject) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_object("nn_session", nn_session);
        flowlang::rustcmd::RustCmd::new("tsmxsj182ee9ac271o2f3").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn settings (&self, settings: Data) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.set_property("settings", settings);
        flowlang::rustcmd::RustCmd::new("knhvsn182f9997b1dxd04").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn spawn (&self, lib: String, ctl: String, cmd: String, args: DataObject) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_object("args", args);
        flowlang::rustcmd::RustCmd::new("tvigvw19268109f0fg2a60").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn timeroff (&self, id: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        flowlang::rustcmd::RustCmd::new("hompli1835678a4efz2").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn timeron (&self, id: String, data: DataObject) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        d.put_object("data", data);
        flowlang::rustcmd::RustCmd::new("spjvvp183568021f1o2").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn uninstall (&self, app: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("app", &app);
        flowlang::rustcmd::RustCmd::new("gttrqg18303bc96c9w898").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn unique_session_id (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("ynpmir183479da2b9r25f8").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn write (&self, lib: String, id: Data, data: DataObject, readers: Data, writers: Data, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.set_property("id", id);
        d.put_object("data", data);
        d.set_property("readers", readers);
        d.set_property("writers", writers);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("yjjxqk18303e75f8atb5a").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl app_service {
    pub fn init (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("mjkrmm183e1fdb2d2r8").execute(d).expect("Rust command execution failed").get_string("a")
    }
}
impl app_util {
    pub fn hash (&self, file: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("file", &file);
        flowlang::rustcmd::RustCmd::new("kgkxpw183664f5554q4").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn init (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("thtpku18366290644p4").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn zip (&self, srcdir: String, destfile: String) -> bool {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("srcdir", &srcdir);
        d.put_string("destfile", &destfile);
        flowlang::rustcmd::RustCmd::new("guuqrj1836147b650zd").execute(d).expect("Rust command execution failed").get_boolean("a")
    }
}
impl agent_llm {
    pub fn ask_llm (&self, prompt: String, system_prompt: Data) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("prompt", &prompt);
        d.set_property("system_prompt", system_prompt);
        flowlang::rustcmd::RustCmd::new("rjuoqv19e8fc5c83ft4").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn tool_loop (&self, prompt: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("prompt", &prompt);
        flowlang::rustcmd::RustCmd::new("lnmvtl19edbeb72a7tc3a").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn chat_llm (&self, messages: DataArray, tools: DataArray) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_array("messages", messages);
        d.put_array("tools", tools);
        flowlang::rustcmd::RustCmd::new("ytohmk19f70b2c09ck7ce2").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl agent_plugin {
    pub fn control_query (&self, message: String, context: DataObject) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("message", &message);
        d.put_object("context", context);
        flowlang::rustcmd::RustCmd::new("innxiu19ebbb8efe6yfdf").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn list_tools (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("sroyxx19ebde8708fk14aa").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn describe_command (&self, command_name: String, lang: String, returntype: String, groups: String, params: DataArray, imports: String, code: String, current_description: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("command_name", &command_name);
        d.put_string("lang", &lang);
        d.put_string("returntype", &returntype);
        d.put_string("groups", &groups);
        d.put_array("params", params);
        d.put_string("imports", &imports);
        d.put_string("code", &code);
        d.put_string("current_description", &current_description);
        flowlang::rustcmd::RustCmd::new("ktoprh19ec10b7907k1b87").execute(d).expect("Rust command execution failed").get_string("a")
    }
}
impl agent_scratch {
    pub fn eval_pshkms19ee68b2a1ct46 (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("ukvisj19ee68b2a21o48").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl agent_archivist {
    pub fn log_turn (&self, venue: String, ask: String, reply: String, tools: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("venue", &venue);
        d.put_string("ask", &ask);
        d.put_string("reply", &reply);
        d.put_string("tools", &tools);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("zktsrl19fb904ad42r2").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn consolidate (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("lwzzvz19fb904b9f0m4").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn queue_status (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("grkhrm19fb91df28dj1").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl dev_dev {
    pub fn check (&self, lib: String, ctl: String, cmd: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        flowlang::rustcmd::RustCmd::new("gsxkwg184e3fc96f9s2e1").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn compile (&self, lib: String, ctl: String, cmd: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        flowlang::rustcmd::RustCmd::new("gjssly1834862d5acg37d9").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn compile_rust (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("mhxogz1858786d9e1scf").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn install_lib (&self, uuid: String, lib: String) -> bool {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("kqgjmx1840a9081cdh172").execute(d).expect("Rust command execution failed").get_boolean("a")
    }
    pub fn lib_archive (&self, lib: String, version: i64) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_int("version", version);
        flowlang::rustcmd::RustCmd::new("uykmrm183dbd15cdeu7b").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn lib_info (&self, lib: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("knwozu1840a764abcu135").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn rebuild_lib (&self, lib: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("yypums1847731c7fap5").execute(d).expect("Rust command execution failed").get_string("a")
    }
}
impl dev_editcommand {
    pub fn compile_command (&self, lib: String, control_name: String, cmd_name: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("control_name", &control_name);
        d.put_string("cmd_name", &cmd_name);
        flowlang::rustcmd::RustCmd::new("wmjmsm19e30a16655r3439").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn delete_command (&self, lib: String, control_id: String, cmd_id: String, nn_sessionid: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("control_id", &control_id);
        d.put_string("cmd_id", &cmd_id);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("zjkntl19e309a2635o3426").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn save_command (&self, lib: String, cmd_id: String, lang: String, code: String, imports: String, returntype: String, params: DataArray, desc: String, groups: String, readers: DataArray, nn_sessionid: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("cmd_id", &cmd_id);
        d.put_string("lang", &lang);
        d.put_string("code", &code);
        d.put_string("imports", &imports);
        d.put_string("returntype", &returntype);
        d.put_array("params", params);
        d.put_string("desc", &desc);
        d.put_string("groups", &groups);
        d.put_array("readers", readers);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("yqvnwh19e30916b04l3410").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn read_command (&self, lib: String, ctl: String, cmd: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        flowlang::rustcmd::RustCmd::new("hkhmnw19e55777c46x41").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn lookup_cmd_id (&self, lib: String, ctl: String, cmd: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        flowlang::rustcmd::RustCmd::new("vpqniv19e558047aeo59").execute(d).expect("Rust command execution failed").get_string("a")
    }
}
impl dev_editcontrol {
    pub fn add_component (&self, lib: String, control_id: String, component_type: String, name: String, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("control_id", &control_id);
        d.put_string("component_type", &component_type);
        d.put_string("name", &name);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("ywmyvk19e2d0d215ai2c5b").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn appdata (&self, data: DataObject) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_object("data", data);
        flowlang::rustcmd::RustCmd::new("vsxqui18332a86185i159").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn get_control (&self, lib: String, id: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("id", &id);
        flowlang::rustcmd::RustCmd::new("ljxttx19e2c502d8bg2ab1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn get_publish_context (&self, lib: String, control_id: String, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("control_id", &control_id);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("lhknos19e2d258cb6w2c94").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn lookup_id (&self, lib: String, name: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("name", &name);
        flowlang::rustcmd::RustCmd::new("ggkslj19e2c58bb61q2ac8").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn publishapp (&self, data: DataObject) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_object("data", data);
        flowlang::rustcmd::RustCmd::new("iwvgmq1835bb194ffo8").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn save_control (&self, lib: String, id: String, html: String, css: String, js: String, groups: String, desc: String, readers: DataArray, inline_data: DataObject, nn_sessionid: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("id", &id);
        d.put_string("html", &html);
        d.put_string("css", &css);
        d.put_string("js", &js);
        d.put_string("groups", &groups);
        d.put_string("desc", &desc);
        d.put_array("readers", readers);
        d.put_object("inline_data", inline_data);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("uwoygr19e2c6bab55r2af6").execute(d).expect("Rust command execution failed").get_string("a")
    }
}
impl dev_github {
    pub fn import (&self, url: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("url", &url);
        flowlang::rustcmd::RustCmd::new("nnjgwh189dcdca95fq7c").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn list (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("lovuhn189dc981ebch2f").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl dev_libsettings {
    pub fn get_library_config (&self, id: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        flowlang::rustcmd::RustCmd::new("gxysqz19721b331c9r54").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn save_library_config (&self, data: DataObject) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_object("data", data);
        flowlang::rustcmd::RustCmd::new("wjhsqs19720f20d2ct8d").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl dev_plugins {
    pub fn list_plugins (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("zvmhyt19763d3e070i43").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl dev_code {
    pub fn list_commands (&self, lib: String, ctl: String) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        flowlang::rustcmd::RustCmd::new("ypmryt19ec1558019m1c2c").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn list_controls (&self, lib: String) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("nhpgow19e9ddf15a2k6").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn list_libraries (&self) -> DataArray {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("mtjtsw19e9dd5bcefg1de5").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn add_library (&self, lib: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("kqzknr19ec8a3ea32offa").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn add_control (&self, lib: String, ctl: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        flowlang::rustcmd::RustCmd::new("lmywwj19ec8a9e2ccm100b").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn upsert_command (&self, lib: String, ctl: String, cmd: String, lang: String, return_type: String, params: DataArray, imports: String, code_body: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_string("lang", &lang);
        d.put_string("return_type", &return_type);
        d.put_array("params", params);
        d.put_string("imports", &imports);
        d.put_string("code_body", &code_body);
        flowlang::rustcmd::RustCmd::new("ovwolr19ec8c38800z1047").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn patch_command_body (&self, lib: String, ctl: String, cmd: String, old_snippet: String, new_snippet: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_string("old_snippet", &old_snippet);
        d.put_string("new_snippet", &new_snippet);
        flowlang::rustcmd::RustCmd::new("slvzur19ed5ad5cc7k2c99").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn read_command (&self, lib: String, ctl: String, cmd: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        flowlang::rustcmd::RustCmd::new("krpzxz19ed5b4aed9v2cad").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn delete_command (&self, lib: String, ctl: String, cmd: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("xqjpyg19ed5c0337dy2cca").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn search_commands (&self, lib: String, ctl: String, query: String) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("query", &query);
        flowlang::rustcmd::RustCmd::new("shlglp19ed5d11bf9i2cf3").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn invoke_command (&self, lib: String, ctl: String, cmd: String, args: DataObject) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_object("args", args);
        flowlang::rustcmd::RustCmd::new("hviwtu19ed5dc7dc5x2d10").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn evaluate_rust (&self, imports: String, code: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("imports", &imports);
        d.put_string("code", &code);
        flowlang::rustcmd::RustCmd::new("nwnguj19ee5977c28s1527").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn read_control_facet (&self, lib: String, ctl: String, facet: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("facet", &facet);
        flowlang::rustcmd::RustCmd::new("vwswvs19f95a61d29j3943").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn patch_control_facet (&self, lib: String, ctl: String, facet: String, old_snippet: String, new_snippet: String, base: String, label: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("facet", &facet);
        d.put_string("old_snippet", &old_snippet);
        d.put_string("new_snippet", &new_snippet);
        d.put_string("base", &base);
        d.put_string("label", &label);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("uyonls19f95a61d2cp3945").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn list_control_patches (&self, lib: String, ctl: String, limit: i64) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_int("limit", limit);
        flowlang::rustcmd::RustCmd::new("zkjqpy19f95a61d2eu3947").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_library_meta (&self, lib: String, desc: String, groups: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("desc", &desc);
        d.put_string("groups", &groups);
        flowlang::rustcmd::RustCmd::new("ouqqjw19f95a61d2fu3949").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_control_meta (&self, lib: String, ctl: String, desc: String, groups: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("desc", &desc);
        d.put_string("groups", &groups);
        flowlang::rustcmd::RustCmd::new("trogig19f95a61d30w394b").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_command_meta (&self, lib: String, ctl: String, cmd: String, desc: String, groups: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_string("desc", &desc);
        d.put_string("groups", &groups);
        flowlang::rustcmd::RustCmd::new("hlsjpo19f95a61d32q394d").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn list_assets (&self, lib: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        flowlang::rustcmd::RustCmd::new("iltsxi19f96bdd724l566d").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn write_asset (&self, lib: String, name: String, content: String, tempfile: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("name", &name);
        d.put_string("content", &content);
        d.put_string("tempfile", &tempfile);
        flowlang::rustcmd::RustCmd::new("vxxmwl19f96bdd725r566f").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn rename_asset (&self, lib: String, from: String, to: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("from", &from);
        d.put_string("to", &to);
        flowlang::rustcmd::RustCmd::new("qosxxk19f96bdd725z5671").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn delete_asset (&self, lib: String, name: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("name", &name);
        flowlang::rustcmd::RustCmd::new("qvipjk19f96bdd726s5673").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn read_flow_body (&self, lib: String, ctl: String, cmd: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        flowlang::rustcmd::RustCmd::new("nprqom19f9925517fn789d").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn write_flow_body (&self, lib: String, ctl: String, cmd: String, body: DataObject, base: String, label: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_object("body", body);
        d.put_string("base", &base);
        d.put_string("label", &label);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("vksvyz19f99255185j789f").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_timer (&self, lib: String, ctl: String, name: String, cmd: String, start: i64, startunit: String, interval: i64, intervalunit: String, repeat: bool, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("name", &name);
        d.put_string("cmd", &cmd);
        d.put_int("start", start);
        d.put_string("startunit", &startunit);
        d.put_int("interval", interval);
        d.put_string("intervalunit", &intervalunit);
        d.put_boolean("repeat", repeat);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("kxtnil19f99e8b05bj9cfd").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn remove_timer (&self, lib: String, ctl: String, name: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("name", &name);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("ppjvhg19f99e8b05fv9cff").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_event_handler (&self, lib: String, ctl: String, name: String, bot: String, event: String, cmd: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("name", &name);
        d.put_string("bot", &bot);
        d.put_string("event", &event);
        d.put_string("cmd", &cmd);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("slwolg19f99e8b060l9d01").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn remove_event_handler (&self, lib: String, ctl: String, name: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("name", &name);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("lyqmyx19f99e8b061q9d03").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn read_control_scene (&self, lib: String, ctl: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        flowlang::rustcmd::RustCmd::new("vtynpg19fa345cd8eyb2ff").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn write_control_scene (&self, lib: String, ctl: String, scene: DataObject, base: String, label: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_object("scene", scene);
        d.put_string("base", &base);
        d.put_string("label", &label);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("ltwuws19fa345cd8erb301").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn delete_library (&self, lib: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("juhgqn19faf571a9az1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn delete_control (&self, lib: String, ctl: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("tjhhxj19faf577471h1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn move_control (&self, lib: String, ctl: String, to_lib: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("to_lib", &to_lib);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("pyjtgq19fb05aeb0cm1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_meta_identity (&self, displayname: String, organization: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("displayname", &displayname);
        d.put_string("organization", &organization);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("pxilgw19fb08d4430k1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn get_meta_identity (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("lgiozw19fb094fdfau1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn unpublish_app (&self, lib: String, app: String, remove_runtime: bool, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("app", &app);
        d.put_boolean("remove_runtime", remove_runtime);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("ijyuys19fb09ff451g1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_module_flag (&self, lib: String, ctl: String, module: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("module", &module);
        flowlang::rustcmd::RustCmd::new("mmtgzg19fb2da96a9g1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_plugin (&self, name: String, target_lib: String, target_ctl: String, plugin_lib: String, plugin_ctl: String, selector: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("name", &name);
        d.put_string("target_lib", &target_lib);
        d.put_string("target_ctl", &target_ctl);
        d.put_string("plugin_lib", &plugin_lib);
        d.put_string("plugin_ctl", &plugin_ctl);
        d.put_string("selector", &selector);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("owxtlg19fb3b6cfd6v1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn remove_plugin (&self, name: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("name", &name);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("znpnwu19fb3b71711u3").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_tags (&self, lib: String, ctl: String, cmd: String, tags: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_string("tags", &tags);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("vsxqpy19fb84a1ba4m1").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn set_groups (&self, lib: String, ctl: String, cmd: String, groups: String, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("ctl", &ctl);
        d.put_string("cmd", &cmd);
        d.put_string("groups", &groups);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("ywwgiq19fb84a4e57h3").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn remember (&self, lib: String, domain: String, entry: DataObject, author: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("lib", &lib);
        d.put_string("domain", &domain);
        d.put_object("entry", entry);
        d.put_string("author", &author);
        flowlang::rustcmd::RustCmd::new("zjqnjs19fb8b46738r1").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl peer_peer {
    pub fn discovery (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("mlrhvx183e6eabd19xb4").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn info (&self, nn_sessionid: String, uuid: Data, salt: Data) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("nn_sessionid", &nn_sessionid);
        d.set_property("uuid", uuid);
        d.set_property("salt", salt);
        flowlang::rustcmd::RustCmd::new("tkwkml18390d46728m8").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn local (&self, request: DataObject, nn_session: DataObject, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_object("request", request);
        d.put_object("nn_session", nn_session);
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("nylhvq183f6b61e43oc2").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn peers (&self) -> DataArray {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("ywokvt1838c110d92l8").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn remote (&self, nn_path: String, nn_params: DataObject, nn_headers: DataObject) -> DataBytes {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("nn_path", &nn_path);
        d.put_object("nn_params", nn_params);
        d.put_object("nn_headers", nn_headers);
        flowlang::rustcmd::RustCmd::new("txnvil183f6ffdf58w1d").execute(d).expect("Rust command execution failed").get_bytes("a")
    }
}
impl peer_reboot {
    pub fn init (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("hygrki1842eac55a9w2a").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn reboot (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("jmhvzv1843439faa0i305").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl peer_service {
    pub fn close_stream (&self, uuid: String, streamid: i64, write: bool) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        d.put_int("streamid", streamid);
        d.put_boolean("write", write);
        flowlang::rustcmd::RustCmd::new("zqxtsm18d3d4ef2b3j101").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn discovery (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("vtxmqr183e5ff3ef5u82").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn exec (&self, uuid: String, app: String, cmd: String, params: DataObject) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        d.put_string("app", &app);
        d.put_string("cmd", &cmd);
        d.put_object("params", params);
        flowlang::rustcmd::RustCmd::new("nmojwg18386b2f0d2n2").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn get_stream (&self, uuid: String, stream_id: i64) -> DataBytes {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        d.put_int("stream_id", stream_id);
        flowlang::rustcmd::RustCmd::new("hlmugl188ab38379arb5").execute(d).expect("Rust command execution failed").get_bytes("a")
    }
    pub fn init (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("grvupm18379e9a159n8").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn listen (&self, ipaddr: String, port: i64) -> i64 {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("ipaddr", &ipaddr);
        d.put_int("port", port);
        flowlang::rustcmd::RustCmd::new("irxuhn18379cef5bcp4").execute(d).expect("Rust command execution failed").get_int("a")
    }
    pub fn listen_udp (&self, ipaddr: String, port: i64) -> i64 {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("ipaddr", &ipaddr);
        d.put_int("port", port);
        flowlang::rustcmd::RustCmd::new("rgxowg183ad6b7a12u6").execute(d).expect("Rust command execution failed").get_int("a")
    }
    pub fn maintenance (&self) -> String {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("rjntml18385b15b5ch0").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn new_stream (&self, uuid: String) -> i64 {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        flowlang::rustcmd::RustCmd::new("myuvuz18d36f76d2cg3").execute(d).expect("Rust command execution failed").get_int("a")
    }
    pub fn session_expire (&self, user: DataObject) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_object("user", user);
        flowlang::rustcmd::RustCmd::new("lvvzvn183bd066566j4").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn stream_write (&self, uuid: String, stream_id: i64, data: DataBytes) -> bool {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        d.put_int("stream_id", stream_id);
        d.put_bytes("data", data);
        flowlang::rustcmd::RustCmd::new("pmumpq18d39a2594cp3").execute(d).expect("Rust command execution failed").get_boolean("a")
    }
    pub fn tcp_connect (&self, uuid: String, ipaddr: String, port: i64) -> bool {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("uuid", &uuid);
        d.put_string("ipaddr", &ipaddr);
        d.put_int("port", port);
        flowlang::rustcmd::RustCmd::new("ltnpiq18385ba6cc7u3").execute(d).expect("Rust command execution failed").get_boolean("a")
    }
    pub fn udp_connect (&self, ipaddr: String, port: i64) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("ipaddr", &ipaddr);
        d.put_int("port", port);
        flowlang::rustcmd::RustCmd::new("gloivk183adf03115od").execute(d).expect("Rust command execution failed").get_object("a")
    }
}
impl scratch_scratch {
    pub fn patch_control_ui (&self, component: String, old_snippet: String, new_snippet: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("component", &component);
        d.put_string("old_snippet", &old_snippet);
        d.put_string("new_snippet", &new_snippet);
        flowlang::rustcmd::RustCmd::new("tnhplg19f7b157521l6fb").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn find_broken_files (&self, dir1: String, dir2: String) -> DataArray {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("dir1", &dir1);
        d.put_string("dir2", &dir2);
        flowlang::rustcmd::RustCmd::new("xvvruo19f821897adz1691").execute(d).expect("Rust command execution failed").get_array("a")
    }
}
impl security_security {
    pub fn current_user (&self, nn_sessionid: String) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("nn_sessionid", &nn_sessionid);
        flowlang::rustcmd::RustCmd::new("ihxsxh18410251dfapf7").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn deleteuser (&self, id: String) -> String {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        flowlang::rustcmd::RustCmd::new("jszjgy1836bfe023ckc").execute(d).expect("Rust command execution failed").get_string("a")
    }
    pub fn groups (&self) -> DataArray {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("qjmvtm1836b1bc850o9").execute(d).expect("Rust command execution failed").get_array("a")
    }
    pub fn init (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("suvlkp1846cfa2235q2c").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn setuser (&self, id: String, displayname: String, password: String, groups: DataArray, keepalive: Data, address: Data, port: Data) -> DataObject {
        let mut d = ndata::dataobject::DataObject::new();
        d.put_string("id", &id);
        d.put_string("displayname", &displayname);
        d.put_string("password", &password);
        d.put_array("groups", groups);
        d.set_property("keepalive", keepalive);
        d.set_property("address", address);
        d.set_property("port", port);
        flowlang::rustcmd::RustCmd::new("soqxoo1836bb51d5dy2").execute(d).expect("Rust command execution failed").get_object("a")
    }
    pub fn users (&self) -> DataObject {
        let d = ndata::dataobject::DataObject::new();
        flowlang::rustcmd::RustCmd::new("ysnihn1836b0814aen5").execute(d).expect("Rust command execution failed").get_object("a")
    }
}

// Standard boilerplate for Newbound controls
var me = this;
var ME = $('#' + me.UUID)[0];

// me.ready is the entry point, called when the control is loaded.
me.ready = function() {
  // In-memory store for the raw conversation history.
  let conversationHistory = [];

  // Get references to the new DOM elements
  var launcher = $(ME).find('.chat-launcher');
  var chatWindow = $(ME).find('.chat-window');
  var minimizeButton = $(ME).find('.minimize-btn');
  var maximizeButton = $(ME).find('.maximize-btn');

  // Get references to the chat functionality elements
  var chatHistory = $(ME).find('.chat-history');
  var chatInput = $(ME).find('.chat-input');
  var sendButton = $(ME).find('.send-button');
  var includeHistoryCheckbox = $(ME).find('.include-history-checkbox'); // New checkbox

  // --- Tool Picker Elements and Logic ---
  var toolsButton = $(ME).find('.tools-btn');
  var toolPickerPanel = $(ME).find('.tool-picker-panel');
  var closeToolsButton = $(ME).find('.close-tools-btn');
  var toolListContainer = $(ME).find('.tool-list');
  const ENABLED_TOOLS_KEY = 'newbound_chat_enabled_tools';

  // --- Event Listeners to toggle chat visibility ---

  launcher.on('click', function() {
    chatWindow.toggleClass('open');
    if (chatWindow.hasClass('open')) {
      chatInput.focus();
    }
  });

  minimizeButton.on('click', function() {
    chatWindow.removeClass('open');
  });

  maximizeButton.on('click', function() {
    if (chatWindow.hasClass('open')) {
      if (chatWindow.height() === 500) {
        chatWindow.height('100vh');
        chatWindow.width('100vw');
        chatWindow.css({ 'bottom': '0', 'right': '0' });
        maximizeButton.html('&or;');
      } else {
        chatWindow.height(500);
        chatWindow.width(370);
        chatWindow.css({ 'bottom': '90px', 'right': '20px' });
        maximizeButton.html('&and;');
      }
    }
  });

  // --- Tool Picker Functionality ---

  toolsButton.on('click', function() {
    toolPickerPanel.addClass('open');
  });

  closeToolsButton.on('click', function() {
    toolPickerPanel.removeClass('open');
  });

  /**
   * Fetches the list of available tools and renders them in the picker.
   */
  function loadAndRenderTools() {
    // Call the real backend command to get the list of tools.
    send_list_tools(function(result) {
      if (result.status !== 'ok') {
        console.error("Failed to fetch tool list:", result.msg || "Unknown error");
        toolListContainer.html('<div style="color: var(--text-muted); padding: 10px;">Error loading tools.</div>');
        return;
      }

      // The actual data is expected in result.tools
      const tools = result.tools || [];
      toolListContainer.empty();
      
      let enabledTools;
      const isFirstLoad = localStorage.getItem(ENABLED_TOOLS_KEY) === null;

      if (isFirstLoad) {
        enabledTools = []; // Default to all tools being disabled
        saveEnabledTools(enabledTools);
      } else {
        enabledTools = getEnabledTools();
      }

      tools.forEach(tool => {
        const toolName = tool.name;
        const toolDesc = tool.description || tool.summary || 'No description available.';
        const isChecked = enabledTools.includes(toolName);
        const toolId = 'tool-checkbox-' + toolName.replace(/[^a-zA-Z0-9]/g, '-');
        
        const toolItem = $(`
          <div class="tool-item">
            <input type="checkbox" id="${toolId}" data-tool-name="${toolName}" ${isChecked ? 'checked' : ''}>
            <label for="${toolId}" title="${$('<div/>').text(toolDesc).html()}">${toolName}</label>
          </div>
        `);
        toolListContainer.append(toolItem);
      });
    });
  }

  /**
   * Gets the list of enabled tool names from localStorage.
   * @returns {string[]} An array of enabled tool names.
   */
  function getEnabledTools() {
    try {
      const stored = localStorage.getItem(ENABLED_TOOLS_KEY);
      return stored ? JSON.parse(stored) : [];
    } catch (e) {
      console.error("Failed to parse enabled tools from localStorage", e);
      return [];
    }
  }

  /**
   * Saves the list of enabled tool names to localStorage.
   * @param {string[]} enabledTools - An array of enabled tool names.
   */
  function saveEnabledTools(enabledTools) {
    try {
      localStorage.setItem(ENABLED_TOOLS_KEY, JSON.stringify(enabledTools));
    } catch (e) {
      console.error("Failed to save enabled tools to localStorage", e);
    }
  }

  // Event listener for toggling a tool's enabled state.
  toolListContainer.on('change', 'input[type="checkbox"]', function() {
    const toolName = $(this).data('tool-name');
    let enabledTools = getEnabledTools();
    if (this.checked) {
      if (!enabledTools.includes(toolName)) {
        enabledTools.push(toolName);
      }
    } else {
      enabledTools = enabledTools.filter(t => t !== toolName);
    }
    saveEnabledTools(enabledTools);
  });

  // --- Core Chat Functionality ---

  // Function to auto-resize the textarea
  function autoResizeTextarea() {
    this.style.height = "auto"; // Reset height to recalculate
    this.style.height = (this.scrollHeight) + "px"; // Set to scroll height
  }

  // Event listener for input to auto-resize the textarea
  chatInput.on("input", autoResizeTextarea);

  /**
   * Strips <think> tags from the bot's raw response for storing in history.
   * @param {string} rawText - The raw text from the bot.
   * @returns {string} - The cleaned text.
   */
  function cleanBotMessageForHistory(rawText) {
    // This regex removes <think>...</think> blocks, including multi-line ones,
    // and any leading/trailing whitespace.
    return rawText.replace(/<think>[\s\S]*?<\/think>\n?/g, '').trim();
  }

  function sendMessage() {
    var messageText = chatInput.val().trim();
    if (messageText === "") return;

    appendMessage(messageText, "user");
    chatInput.val("");
    chatInput[0].style.height = "auto"; // Reset height after sending

    var context = getEditorContents();
    context.enabled_tools = getEnabledTools();

    // If the history checkbox is checked, pass the conversation history.
    // Otherwise, clear the history to start a fresh conversation context.
    if (includeHistoryCheckbox.is(':checked')) {
      context.chat_history = conversationHistory;
    } else {
      conversationHistory = [];
    }

    console.log("Context being sent to backend:", context);

    send_control_query(messageText, context, function(result) {
      // On successful response, add both the user's message and the bot's raw
      // response to our in-memory history for the next turn.
      if (result.status === 'ok') {
        conversationHistory.push({ role: 'user', content: messageText });
        var cleanMsg = cleanBotMessageForHistory(result.msg);
        conversationHistory.push({ role: 'assistant', content: cleanMsg });
        var formattedMsg = formatBotMessage(result.msg);
        appendMessage(formattedMsg, 'bot');
      } else {
        var errorMsg = "Error: " + (result.msg || "Unknown error");
        appendMessage(errorMsg, 'bot');
        // On error, we don't add anything to the history.
      }
    });
  }

  function appendMessage(content, sender) {
    var messageClass = (sender === 'user') ? 'user-message' : 'bot-message';
    var messageElement = $('<div class="message"></div>').addClass(messageClass);
    if (sender === 'user') {
      messageElement.text(content);
    } else {
      messageElement.html(content);
    }
    chatHistory.append(messageElement);
    chatHistory.scrollTop(chatHistory[0].scrollHeight);
  }

  /**
   * Formats a bot's response, replacing code blocks with interactive placeholders.
   * @param {string} text - The raw text from the bot.
   * @returns {string} - The formatted HTML string.
   */
  function formatBotMessage(text) {
    var html = '';
    var current = null;
    var currentType = null;
    var currentThought = null;
    let lines = text.split("\n");
    for (var i in lines) {
      var line = lines[i];
      if (line.startsWith("</think>")) {
        var check = $('<div>').html(currentThought).text().trim();
        if (check != "") {
          html += `
<button class="showthinkingbutton" onclick="$(this).css('display', 'none').next().css('display', 'block').next().css('display', 'block');">show thinking</button><div style="display:none;">` + currentThought + `</div><button class="hidethinkingbutton" onclick="$(this).css('display', 'none').prev().css('display', 'none').prev().css('display', 'block');" style="display:none;">hide thinking</button>`;
        }
        currentThought = null;
      } else if (line.startsWith("<think>")) {
        currentThought = '<div class="response-line">' + line.substring(7) + '</div>';
      } else if (line.startsWith("```")) {
        if (current != null) {
          var chunkName = currentType;
          var escapedCode = $('<div>').text(current).html().replaceAll("'", "&#39;");
          html += `<div class="bot-code-block">
<p>Code block: <strong>${chunkName}</strong></p>
<button class="view-code-btn" data-chunk="${chunkName}" data-code='${escapedCode}'>
<svg xmlns="[http://www.w3.org/2000/svg](http://www.w3.org/2000/svg)" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>
<br>
View Code
</button>
</div>`;
          current = null;
        } else {
          current = "";
          currentType = line.substring(3).trim();
        }
      } else if (currentThought != null) {
        line = $('<div>').text(line).html();
        currentThought += '<div class="response-line">' + line + '</div>';
      } else if (current != null) {
        if (current != "") { current += "\n"; }
        current += line;
      } else {
        line = $('<div>').text(line).html();
        html += '<div class="response-line">' + line + '</div>';
      }
    }
    return html;
  }

  // --- Event Listeners for sending messages ---
  sendButton.on('click', sendMessage);
  chatInput.on('keydown', function(event) {
    if (event.which === 13 && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  });

  // --- Delegated Event Listener for Code Modals ---
  chatHistory.on('click', '.view-code-btn', function() {
    let code = $(this).data('code');
    let chunk = $(this).data('chunk');
    if (typeof code == 'object') { code = JSON.stringify(code); } 
    showCodeModal(code, chunk);
  });

  // --- Initial Setup ---
  loadAndRenderTools();
  
  function ddci(){
    console.log("trying to populate command editor");
    if ($(".api-editcommand")[0].api) me.doCmdInit();
    else setTimeout(ddci, 1000);
  }
  
  ddci();
};  
  
me.doCmdInit = function(){  
  console.log("populating command editor");
  var CMDDIV = $(".api-editcommand")[0];
  var cmddiv = CMDDIV.api;
  
  var el = $('<button class="boo">generate</button>');
  var statusIndicator = $('<span class="status-indicator" style="margin-left: 10px; font-size: 0.9em; color: #888;"></span>').hide(); // Initially hidden
  
  $("#ecmd_desc").prev().append(el);
  $("#ecmd_desc").prev().append(statusIndicator); // Append the status indicator next to the button
  
  el.click(function(){
    // Show status indicator
    statusIndicator.text("Generating...").show();
    
    // Gather all necessary data from the DOM to send to the Rust command
    const { code, imports, returntype, lang, params } = cmddiv.getCommandData();
    const commandName = CMDDIV.DATA.name;
    const currentDescription = $("#ecmd_desc").val().trim();
    const groups = $("#ecmd_groups").val().trim();

    // Call the new back-end Rust command "describe_command"
    // The arguments match the parameters of the Rust function.
    send_describe_command(
      commandName,
      lang,
      returntype,
      groups,
      params, // This array of JS objects will be automatically converted to a DataArray of DataObjects for Rust
      imports,
      code,
      currentDescription,
      function(result) {
        // Hide status indicator regardless of outcome
        statusIndicator.hide(); 

        if (result.status !== "ok") {
          // Handle any errors returned from the Rust command
          alert("Error generating description: " + (result.msg || "An unknown error occurred."));
        } else {
          // The Rust command returns a String, which will be in result.msg
          $("#ecmd_desc").val(result.msg);
        }
      }
    );
  });
  
  const params = cmddiv.getCommandData().params;
  params.forEach((param, i) => { // Use forEach for proper scope capturing
    console.log(i+" / "+JSON.stringify(param));
    
    const descinput = $("#param-desc-"+i);
    
    // Add "generate" button to each parameter description.
    const elParam = $('<button class="boo">generate</button>');
    const statusIndicatorParam = $('<span class="status-indicator" style="margin-left: 10px; font-size: 0.9em; color: #888;"></span>').hide();
    
    // Append to the element before the description input (likely the label)
    descinput.parent().prev().append(elParam);
    descinput.parent().prev().append(statusIndicatorParam);
    
    elParam.click(function() {
      statusIndicatorParam.text("Generating...").show();
      
      const { code, imports, returntype, lang, params: allCommandParams } = cmddiv.getCommandData(); // Rename params to avoid conflict
      const commandName = CMDDIV.DATA.name;
      const groups = $("#ecmd_groups").val().trim();
      
      const currentParamDescription = descinput.val().trim(); // Use the specific descinput for this parameter
      
      // Call a new back-end Rust command "describe_parameter"
      // Note: You would need to create a corresponding `describe_parameter` Rust command.
      send_describe_parameter(
        commandName,
        param.name, // Specific parameter name
        param.type, // Specific parameter type
        currentParamDescription, // Current description of this parameter
        lang,
        returntype,
        groups,
        allCommandParams, // Pass all parameters as context
        imports,
        code,
        function(result) {
          statusIndicatorParam.hide();
          if (result.status !== "ok") {
            alert("Error generating parameter description: " + (result.msg || "An unknown error occurred."));
          } else {
            descinput.val(result.msg); // Update the specific parameter"s description
          }
        }
      );
    });
  });

  
  
  
  
  
};

function showCodeModal(codeBlock, headChunk) {
  if (!codeBlock) return;
  var modal = $('<div>').attr('id', 'code-block-modal').css({
    position: 'fixed', top: '50%', left: '50%', transform: 'translate(-50%, -50%)',
    background: 'var(--bg-deep)', padding: '20px', border: '1px solid var(--bg-header)',
    borderRadius: '8px', maxWidth: '80vw', maxHeight: '80vh',
    overflow: 'auto', zIndex: '10000', color: 'var(--text-main)',
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif'
  });
  var modalBody = $('<div>').css({ whiteSpace: 'pre-wrap', wordWrap: 'break-word' });
  var codeBlockContainer = $('<div>').css({
    border: '1px solid var(--bg-header)', padding: '10px', borderRadius: '5px',
    backgroundColor: 'var(--bg-header)', position: 'relative'
  });
  var headChunkDisplay = $('<div>').text(headChunk).css({
    paddingBottom: '10px',
    fontWeight: 'bold',
    color: 'var(--text-muted)'
  });
  var codeElement = $('<pre>').text(codeBlock).css({
    backgroundColor: 'var(--bg-deep)', color: 'var(--text-main)', padding: '10px',
    borderRadius: '4px', fontFamily: 'monospace', maxHeight: '60vh', overflow: 'auto'
  });
  var buttonsContainer = $('<div>').css({ display: 'flex', justifyContent: 'space-between', marginTop: '10px' });
  var closeButton = $('<button>').text('Close').css({
    padding: '8px 12px', backgroundColor: 'var(--text-muted)', color: 'var(--text-main)',
    border: 'none', borderRadius: '4px', cursor: 'pointer', transition: 'background-color 0.2s ease'
  }).on('mouseover', function() { $(this).css('backgroundColor', '#7a88b8'); })
    .on('mouseout', function() { $(this).css('backgroundColor', 'var(--text-muted)'); });
  var actionButtons = $('<div>');
  var copyButton = $('<button>').text('Copy').css({
    padding: '8px 12px', backgroundColor: 'var(--accent-cyan)', color: 'var(--text-on-accent)',
    border: 'none', borderRadius: '4px', cursor: 'pointer', fontWeight: 'bold'
  }).on('click', function() {
    navigator.clipboard.writeText(codeBlock).then(function() {
      document.body.api.ui.snackbarMsg('Code copied to clipboard!');
    }).catch(function(err) { console.error('Failed to copy code: ', err); });
  });
  actionButtons.append(copyButton);
  if (headChunk) {
    var replaceButton = $('<button>').css({
      padding: '8px 12px', backgroundColor: 'var(--accent-cyan)', color: 'var(--text-on-accent)',
      border: 'none', borderRadius: '4px', cursor: 'pointer', fontWeight: 'bold', marginLeft: '10px'
    });

    var isCommand = headChunk.includes(':') && headChunk.split(':').length > 2;
    var isBehavior = headChunk.toLowerCase().startsWith('behavior');

    if (isCommand) {
      var parts = headChunk.split(':');
      var cmdParts = parts[2].split('.');
      var cmdName = cmdParts[0];
      var cmdLang = cmdParts[1];
      replaceButton.text('Replace Command (' + cmdLang + ')');
      replaceButton.on('click', function() {
        if (document.body.api.isCommandEditorActive()) {
          var editorEl = $('.api-editcommand')[0];
          if (editorEl && editorEl.DATA && editorEl.DATA.name === cmdName) {
            if (editorEl.cm) {
              editorEl.cm.setValue(codeBlock);
              modal.remove();
              document.body.api.ui.snackbarMsg('Command code replaced for ' + cmdName);
            } else {
              document.body.api.ui.snackbarMsg('Error: Could not find command editor instance.');
            }
          } else {
            document.body.api.ui.snackbarMsg('Error: Active command editor (' + (editorEl && editorEl.DATA ? editorEl.DATA.name : 'unknown') + ') does not match target (' + cmdName + ').');
          }
        } else {
          document.body.api.ui.snackbarMsg('Error: Command editor is not active.');
        }
      });
      actionButtons.append(replaceButton);
    } else if (isBehavior) {
        replaceButton.text('Replace Behavior');
        replaceButton.on('click', function() {
            const behaviorEditorTextarea = $('.x3dbehavior-editor .x3dbehavior-src-js')[0];
            if (behaviorEditorTextarea && behaviorEditorTextarea.cm) {
                behaviorEditorTextarea.cm.setValue(codeBlock);
                modal.remove();
                document.body.api.ui.snackbarMsg('3D Behavior code replaced.');
            } else {
                document.body.api.ui.snackbarMsg('Error: 3D Behavior editor is not active.');
            }
        });
        actionButtons.append(replaceButton);
    } else {
      // Check for control file format (e.g., lib:ctl.js, javascript, js, html, css)
      var potentialType = headChunk;
      if (potentialType.includes('.')) {
        potentialType = potentialType.substring(potentialType.lastIndexOf('.') + 1);
      }
      if (potentialType === 'javascript') {
        potentialType = 'js';
      }

      if (['html', 'css', 'js'].includes(potentialType)) {
        var chunkType = potentialType;
        replaceButton.text('Replace ' + chunkType.toUpperCase());
        replaceButton.on('click', function() {
          var editorSelector = '.' + chunkType + '-textarea';
          var editor = $(editorSelector)[0];
          if (editor && editor.cm) {
            editor.cm.setValue(codeBlock);
            modal.remove();
            document.body.api.ui.snackbarMsg(chunkType.toUpperCase() + ' code replaced.');
          } else {
            document.body.api.ui.snackbarMsg('Could not find editor for: ' + chunkType);
          }
        });
        actionButtons.append(replaceButton);
      }
    }
  }
  buttonsContainer.append(closeButton, actionButtons);
  codeBlockContainer.append(headChunkDisplay, codeElement, buttonsContainer);
  modalBody.append(codeBlockContainer);
  modal.append(modalBody);
  var closeModal = function() {
    modal.remove();
    $(document).off('keydown.modalClose');
  };
  closeButton.on('click', closeModal);
  modal.on('click', function(e) { if (e.target === modal[0]) { closeModal(); } });
  $(document).on('keydown.modalClose', function(e) { if (e.key === 'Escape') { closeModal(); } });
  
  $('body').append(modal);
}

function getEditorContents() {
    // Query for the state of the checkboxes within this function
    const includeHtml = $(ME).find('#include-html-checkbox').is(':checked');
    const includeCss = $(ME).find('#include-css-checkbox').is(':checked');
    const includeJs = $(ME).find('#include-js-checkbox').is(':checked');
    const includeData = $(ME).find('#include-data-checkbox').is(':checked');
    const includeCommand = $(ME).find('#include-command-checkbox').is(':checked');
    const includeBehavior = $(ME).find('#include-behavior-checkbox').is(':checked');
    
    const contents = { html: null, css: null, js: null, data: null };
    const getValueFromEditor = (selector) => {
        const element = $(selector)[0];
        if (element && element.cm) {
            return element.cm.getValue();
        }
        return null;
    };

    if (includeHtml) contents.html = getValueFromEditor('.html-textarea');
    if (includeCss) contents.css = getValueFromEditor('.css-textarea');
    if (includeJs) contents.js = getValueFromEditor('.js-textarea');
    if (includeData) contents.data = getValueFromEditor('.data-textarea');

    // These are meta-data for the control context, only include if a control part is included.
    if (contents.html || contents.css || contents.js || contents.data) {
        contents.lib = $('.ctl-lib').text();
        contents.ctl = $('.ctl-name').text();
    }

    // Check for active 3D Behavior Editor
    if (includeBehavior) {
        const is3DEditorActive = $('.navbar-tab[data-id="tab2"]').hasClass('selected');
        const behaviorEditor = $('.x3dbehavior-editor');

        if (is3DEditorActive && behaviorEditor.is(':visible')) {
            const behaviorTextarea = behaviorEditor.find('.x3dbehavior-src-js')[0];
            if (behaviorTextarea && behaviorTextarea.cm) {
                contents.behaviorCode = behaviorTextarea.cm.getValue();
                try {
                    const edit3dApi = $('.three-main')[0].api;
                    if (edit3dApi && edit3dApi.current_behavior) {
                        contents.behaviorName = edit3dApi.current_behavior[0].name;
                    }
                } catch (e) {
                    console.log("Could not get 3D behavior name from API.");
                }
            }
        }
    }

    // Check for active command editor
    if (includeCommand && document.body.api.isCommandEditorActive()) {
        var el = $('.api-editcommand')[0];
        var data = el.DATA;
        contents.cmdname = data.name;
        var cmddata = el.api.getCommandData();
        contents.cmdlang = cmddata.lang == "rust" ? "rs" : cmddata.lang;
        contents.cmd = cmddata.code;
        contents.cmdreturn = cmddata.returntype;
        contents.cmdimport = cmddata.imports;
        contents.cmdparams = cmddata.params;
    }
    return contents;
}

var me = this;
var ME = $('#' + me.UUID)[0];

me.ready = function() {
  var $chatHistory = $(ME).find('#chat-history');
  var $myprompt = $(ME).find('#myprompt');
  var $sysprompt = $(ME).find('#sysprompt');
  var $sysPromptArea = $(ME).find('#sys-prompt-area');
  var $sysToggle = $(ME).find('#sys-toggle');
  var $sendBtn = $(ME).find('#mybutton');

  // Toggle System Prompt
  $sysToggle.click(function() {
    if ($sysPromptArea.is(':visible')) {
      $sysPromptArea.hide();
      $sysToggle.text('⚙️ System Prompt');
    } else {
      $sysPromptArea.show();
      $sysToggle.text('⚙️ Hide System Prompt');
    }
  });

  // Auto-resize textarea
  $myprompt.on('input', function() {
    this.style.height = 'auto';
    this.style.height = (this.scrollHeight) + 'px';
  });

  // Helper to add message to chat
  function addMessage(text, type) {
    var msgDiv = $('<div>').addClass('message').addClass(type);
    // If it's a JSON object, stringify it nicely, otherwise just text
    if (typeof text === 'object') {
      msgDiv.append($('<code>').text(JSON.stringify(text, null, 2)));
    } else {
      msgDiv.text(text);
    }
    $chatHistory.append(msgDiv);
    $chatHistory.scrollTop($chatHistory[0].scrollHeight);
  }

  // Send Button Click
  $sendBtn.click(function() {
    var userText = $myprompt.val().trim();
    if (!userText) return;

    // Disable button and clear input
    $sendBtn.prop('disabled', true).text('...');
    $myprompt.val('').css('height', 'auto');
    
    // Add user message to UI immediately
    addMessage(userText, 'user');

    var sysText = $sysprompt.val().trim();
    var sysPrompt = (sysText === "") ? null : sysText;
    
    // Call Backend
    send_tool_loop(userText, function(result) {
    //send_ask_llm(userText, sysPrompt, function(result) {
      $sendBtn.prop('disabled', false).text('Send');
      
      if (result.messages){
        for (var i in result.messages) {
          var mm = JSON.stringify(result.messages[i]);
          addMessage(mm, 'system');
        }
      }

      if (result.status === 'ok') {
        // Check if data is a string or object
        var responseText = result.msg || (result.data ? JSON.stringify(result.data) : "No response data");
        addMessage(responseText, 'system');
      } else {
        addMessage("Error: " + (result.msg || "Unknown error"), 'error');
      }
    });
  });

  // Allow Enter key to send (Shift+Enter for new line)
  $myprompt.keydown(function(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      $sendBtn.click();
    }
  });
};
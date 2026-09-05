/**
 * RashBot Chat Widget for kprsnt.in
 * Floating AI chatbot powered by Gemini + RAG retrieval
 */
(function () {
    'use strict';

    const SUGGESTIONS = [
        "What projects has Prashanth built?",
        "Tell me about BrandXY",
        "What are his AI skills?",
        "Drug discovery projects?",
        "What's MyLocalCLI?"
    ];

    let isOpen = false;
    let isLoading = false;
    let history = [];

    /** Inject chat widget HTML */
    function init() {
        const html = `
        <button class="chat-bubble" id="chat-bubble" aria-label="Chat with AI">
            <span class="pulse-ring"></span>
            <i class="fas fa-comment-dots"></i>
        </button>
        <div class="chat-panel" id="chat-panel">
            <div class="chat-header">
                <div class="chat-header-avatar">🤖</div>
                <div class="chat-header-info">
                    <h4>RashBot</h4>
                    <p>AI-powered · Ask me anything</p>
                </div>
            </div>
            <div class="chat-messages" id="chat-messages">
                <div class="chat-msg bot">
                    Hey! I'm RashBot — Prashanth's AI assistant. Ask me anything about his projects, skills, or experience. 🚀
                </div>
            </div>
            <div class="chat-suggestions" id="chat-suggestions">
                ${SUGGESTIONS.map(s => `<div class="chat-suggestion-pill" onclick="window._chatWidget.askSuggestion('${s}')">${s}</div>`).join('')}
            </div>
            <div class="chat-input-area">
                <input type="text" class="chat-input" id="chat-input" placeholder="Ask RashBot anything..." maxlength="300" />
                <button class="chat-send" id="chat-send" aria-label="Send message">
                    <i class="fas fa-paper-plane"></i>
                </button>
            </div>
        </div>`;

        const container = document.createElement('div');
        container.innerHTML = html;
        document.body.appendChild(container);

        // Event listeners
        document.getElementById('chat-bubble').addEventListener('click', togglePanel);
        document.getElementById('chat-send').addEventListener('click', sendMessage);
        document.getElementById('chat-input').addEventListener('keydown', function (e) {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                sendMessage();
            }
        });
    }

    /** Toggle chat panel */
    function togglePanel() {
        isOpen = !isOpen;
        const panel = document.getElementById('chat-panel');
        const bubble = document.getElementById('chat-bubble');

        panel.classList.toggle('open', isOpen);
        bubble.classList.toggle('open', isOpen);
        bubble.innerHTML = isOpen
            ? '<i class="fas fa-times"></i>'
            : '<span class="pulse-ring"></span><i class="fas fa-comment-dots"></i>';

        if (isOpen) {
            setTimeout(() => document.getElementById('chat-input').focus(), 300);
        }
    }

    /** Add message to chat */
    function addMessage(text, isUser) {
        const messages = document.getElementById('chat-messages');
        const msg = document.createElement('div');
        msg.className = `chat-msg ${isUser ? 'user' : 'bot'}`;
        msg.textContent = text;
        messages.appendChild(msg);
        messages.scrollTop = messages.scrollHeight;
    }

    /** Show typing indicator */
    function showTyping() {
        const messages = document.getElementById('chat-messages');
        const typing = document.createElement('div');
        typing.className = 'chat-typing';
        typing.id = 'chat-typing';
        typing.innerHTML = '<span></span><span></span><span></span>';
        messages.appendChild(typing);
        messages.scrollTop = messages.scrollHeight;
    }

    /** Remove typing indicator */
    function hideTyping() {
        const el = document.getElementById('chat-typing');
        if (el) el.remove();
    }

    /** Send message to API */
    async function sendMessage() {
        if (isLoading) return;

        const input = document.getElementById('chat-input');
        const query = input.value.trim();
        if (!query) return;

        input.value = '';
        addMessage(query, true);

        // Hide suggestions after first message
        const suggestions = document.getElementById('chat-suggestions');
        if (suggestions) suggestions.style.display = 'none';

        isLoading = true;
        document.getElementById('chat-send').disabled = true;
        showTyping();

        // Build conversation history (last 4 exchanges)
        history.push({ role: 'user', content: query });
        const recentHistory = history.slice(-8);

        try {
            const response = await fetch('/api/chat_agent', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    message: query,
                    history: recentHistory,
                    send_email: false
                })
            });

            hideTyping();

            if (!response.ok) {
                addMessage('Connection error. Please try again.', false);
                isLoading = false;
                document.getElementById('chat-send').disabled = false;
                document.getElementById('chat-input').focus();
                return;
            }

            const data = await response.json();
            const assistantMessage = data.response || "Sorry, I couldn't generate a response.";
            
            // Render the full message using innerHTML to preserve linebreaks
            const messages = document.getElementById('chat-messages');
            const msgObj = document.createElement('div');
            msgObj.className = 'chat-msg bot';
            msgObj.innerHTML = assistantMessage.replace(/\n/g, '<br>');
            messages.appendChild(msgObj);
            messages.scrollTop = messages.scrollHeight;

            history.push({ role: 'assistant', content: assistantMessage });
            
        } catch (err) {
            hideTyping();
            addMessage('Connection error. Please try again.', false);
        }

        isLoading = false;
        document.getElementById('chat-send').disabled = false;
        document.getElementById('chat-input').focus();
    }

    /** Handle suggestion click */
    function askSuggestion(text) {
        document.getElementById('chat-input').value = text;
        sendMessage();
    }

    // Public API
    window._chatWidget = { askSuggestion };

    // Init on DOM ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();

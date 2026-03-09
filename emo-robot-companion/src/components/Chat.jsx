import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Send, X, Wrench, CheckCircle, Cpu, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

const stepIcon = (type) => {
    switch (type) {
        case 'thinking': return <Cpu size={11} className="text-yellow-400 mt-0.5 shrink-0" />;
        case 'tool_call': return <Wrench size={11} className="text-purple-400 mt-0.5 shrink-0" />;
        case 'tool_result': return <CheckCircle size={11} className="text-green-400 mt-0.5 shrink-0" />;
        default: return null;
    }
};

const Chat = ({ onClose, onMoodChange }) => {
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [messages, setMessages] = useState([
        { id: 1, role: 'assistant', text: "Hi! I'm Emo. How can I help? 🤖", steps: [] }
    ]);
    const [modelStatus, setModelStatus] = useState({ small_loaded: false, large_loaded: false });
    const messagesEndRef = useRef(null);

    // Auto-scroll to bottom
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    // Check model status on mount
    useEffect(() => {
        const checkStatus = async () => {
            try {
                const status = await invoke('get_model_status');
                setModelStatus(status);
            } catch (_) { }
        };
        checkStatus();
    }, []);

    const ensureModelLoaded = async () => {
        if (!modelStatus.small_loaded) {
            setMessages(prev => [...prev, {
                id: Date.now(),
                role: 'system',
                text: '⚙️ Loading AI model — this may take a moment...',
                steps: []
            }]);
            await invoke('load_model');
            setModelStatus(prev => ({ ...prev, small_loaded: true }));
        }
    };

    const handleSend = async () => {
        if (!input.trim() || isLoading) return;

        const userText = input.trim();
        const userMsg = { id: Date.now(), role: 'user', text: userText, steps: [] };
        setMessages(prev => [...prev, userMsg]);
        setInput('');
        setIsLoading(true);
        onMoodChange?.('thinking');

        try {
            await ensureModelLoaded();

            // Use agent_run — returns array of AgentStep objects
            let agentSteps;
            try {
                agentSteps = await invoke('agent_run', { prompt: userText });
            } catch (e) {
                if (e.toString().includes('Model not loaded') || e.toString().includes('No model')) {
                    await invoke('load_model');
                    setModelStatus(prev => ({ ...prev, small_loaded: true }));
                    agentSteps = await invoke('agent_run', { prompt: userText });
                } else {
                    throw e;
                }
            }

            // Separate intermediate steps from final response
            const intermediateSteps = agentSteps.filter(s => s.step_type !== 'response');
            const finalStep = agentSteps.find(s => s.step_type === 'response');

            setMessages(prev => [...prev, {
                id: Date.now() + 1,
                role: 'assistant',
                text: finalStep?.content ?? '...',
                steps: intermediateSteps,
            }]);

            onMoodChange?.('happy');
        } catch (error) {
            console.error('Agent error:', error);
            setMessages(prev => [...prev, {
                id: Date.now() + 2,
                role: 'assistant',
                text: `Sorry, I ran into an issue: ${error}`,
                steps: []
            }]);
            onMoodChange?.('idle');
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <motion.div
            className="w-full h-full flex flex-col relative"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
        >
            {/* Header */}
            <div className="flex items-center justify-between px-3 pt-3 pb-2 border-b border-white/5">
                <div className="flex items-center gap-2">
                    <span className="text-[11px] font-semibold text-white/70 tracking-wide">EMO CHAT</span>
                    <div className="flex gap-1">
                        <span className={`w-1.5 h-1.5 rounded-full ${modelStatus.small_loaded ? 'bg-green-400' : 'bg-white/20'}`} title="0.5B model" />
                        <span className={`w-1.5 h-1.5 rounded-full ${modelStatus.large_loaded ? 'bg-blue-400' : 'bg-white/20'}`} title="1.5B model" />
                    </div>
                </div>
                {onClose && (
                    <button onClick={onClose} className="p-1 rounded-full hover:bg-white/10 text-white/40 hover:text-white transition-colors">
                        <X size={12} />
                    </button>
                )}
            </div>

            {/* Messages */}
            <div className="flex-1 overflow-y-auto px-3 py-2 space-y-3 custom-scrollbar">
                <AnimatePresence initial={false}>
                    {messages.map((msg) => (
                        <motion.div
                            key={msg.id}
                            initial={{ opacity: 0, y: 8, scale: 0.96 }}
                            animate={{ opacity: 1, y: 0, scale: 1 }}
                            transition={{ type: 'spring', stiffness: 500, damping: 30 }}
                            className={`flex flex-col ${msg.role === 'user' ? 'items-end' : 'items-start'}`}
                        >
                            {/* Intermediate steps (thinking / tool calls) */}
                            {msg.steps && msg.steps.length > 0 && (
                                <div className="mb-1 w-full space-y-0.5">
                                    {msg.steps.map((step, i) => (
                                        <div key={i} className="flex items-start gap-1.5 text-[10px] text-white/40 pl-1">
                                            {stepIcon(step.step_type)}
                                            <span className="font-mono leading-tight">{step.content}</span>
                                        </div>
                                    ))}
                                </div>
                            )}

                            {/* Message bubble */}
                            {msg.role === 'system' ? (
                                <div className="w-full text-center">
                                    <span className="text-[10px] text-white/30 italic">{msg.text}</span>
                                </div>
                            ) : (
                                <div
                                    className={`max-w-[90%] px-3 py-2 rounded-2xl text-[12px] leading-relaxed
                                        ${msg.role === 'user'
                                            ? 'bg-emo-cyan/20 text-white rounded-br-sm border border-emo-cyan/30'
                                            : 'bg-white/5 text-white/90 rounded-bl-sm border border-white/10'
                                        }`}
                                >
                                    {msg.text}
                                </div>
                            )}
                        </motion.div>
                    ))}
                </AnimatePresence>

                {/* Typing indicator */}
                {isLoading && (
                    <motion.div
                        initial={{ opacity: 0, scale: 0.8 }}
                        animate={{ opacity: 1, scale: 1 }}
                        className="flex items-start gap-2"
                    >
                        <div className="px-3 py-2 bg-white/5 rounded-2xl rounded-bl-sm border border-white/10 flex items-center gap-1.5">
                            <Loader2 size={11} className="text-emo-cyan animate-spin" />
                            <span className="text-[11px] text-white/50">Thinking...</span>
                        </div>
                    </motion.div>
                )}

                <div ref={messagesEndRef} />
            </div>

            {/* Input */}
            <div className="px-3 pb-3 pt-2 border-t border-white/5">
                <div className="relative">
                    <input
                        type="text"
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={(e) => e.key === 'Enter' && handleSend()}
                        placeholder="Ask Emo anything..."
                        disabled={isLoading}
                        className="w-full bg-black/40 border border-white/10 rounded-full py-2 pl-4 pr-9 text-[12px] text-white placeholder-white/25 focus:outline-none focus:border-emo-cyan/50 focus:ring-1 focus:ring-emo-cyan/30 transition-all backdrop-blur-sm disabled:opacity-50"
                    />
                    <button
                        onClick={handleSend}
                        disabled={isLoading || !input.trim()}
                        className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 rounded-full hover:bg-emo-cyan/20 text-emo-cyan/70 hover:text-emo-cyan transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
                    >
                        <Send size={12} />
                    </button>
                </div>
            </div>
        </motion.div>
    );
};

export default Chat;

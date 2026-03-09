import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Mic, Menu, X, Disc, Settings2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import Eyes from './Eyes';
import Chat from './Chat';
import Settings from './Settings';

const EmoRobot = () => {
    const [mood, setMood] = useState('idle');
    const [isHovered, setIsHovered] = useState(false);
    const [isExpanded, setIsExpanded] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [voiceStatus, setVoiceStatus] = useState('stopped'); // stopped | ready | listening | processing | error
    const [errorMessage, setErrorMessage] = useState(null);

    const moodRef = useRef('idle');
    const voiceStatusRef = useRef('stopped');

    useEffect(() => { moodRef.current = mood; }, [mood]);
    useEffect(() => { voiceStatusRef.current = voiceStatus; }, [voiceStatus]);

    // Toggle expand
    const toggleExpand = () => setIsExpanded(!isExpanded);

    // Handle mood change (listening toggle)
    const toggleListening = async () => {
        try {
            if (voiceStatus === 'stopped' || voiceStatus === 'error') {
                console.log("Starting voice recognition...");
                setVoiceStatus('starting');
                setMood('listening');
                await invoke('start_listening');
            } else {
                console.log("Stopping voice recognition...");
                setVoiceStatus('stopped');
                setMood('idle');
                await invoke('stop_listening');
            }
            setErrorMessage(null);
        } catch (error) {
            console.error("Failed to toggle listening:", error);
            setVoiceStatus('error');
            setErrorMessage(error.toString());
            setMood('idle');
        }
    };

    useEffect(() => {
        let unlistenStt = null;
        let unlistenVoiceReady = null;
        let unlistenVoiceProcessing = null;
        let unlistenVoiceError = null;

        const processInput = async (text) => {
            const currentMood = moodRef.current;
            console.log(`Processing "${text}" in mood: ${currentMood}`);

            // Always process if we got text (wake word handled in backend now)
            if (text.toLowerCase().includes("hey emo") ||
                text.toLowerCase().includes("hi emo") ||
                text.toLowerCase().includes("emo")) {

                console.log("Wake word detected!");
                setMood('happy');

                // Extract command after wake word
                const parts = text.toLowerCase().split(/emo/);
                const cmdPart = parts[parts.length - 1].trim();

                if (cmdPart.length > 2) {
                    await handleCommand(cmdPart);
                } else {
                    setMood('listening');
                    try {
                        await invoke('speak', { text: "Yes? I'm listening." });
                    } catch (e) {
                        console.log("TTS not available, continuing silently");
                    }
                }
            } else if (voiceStatusRef.current === 'listening' || voiceStatusRef.current === 'ready') {
                // Direct command without wake word
                await handleCommand(text);
            }
        };

        const handleCommand = async (command) => {
            if (command.length < 2) return;

            console.log("Handling command:", command);
            setMood('thinking');
            setVoiceStatus('processing');

            try {
                // First ensure model is loaded
                try {
                    const response = await invoke('generate_text', { prompt: command });
                    console.log("AI Response:", response);
                    setMood('happy');

                    // Try to speak, but don't fail if TTS unavailable
                    try {
                        await invoke('speak', { text: response });
                    } catch (ttsError) {
                        console.log("TTS unavailable, showing text only");
                    }

                    setVoiceStatus('ready');
                    setMood('listening');
                } catch (e) {
                    if (e.toString().includes("Model not loaded")) {
                        console.log("Loading model...");
                        await invoke('load_model');
                        const response = await invoke('generate_text', { prompt: command });
                        setMood('happy');
                        try {
                            await invoke('speak', { text: response });
                        } catch (ttsError) {
                            console.log("TTS unavailable");
                        }
                        setVoiceStatus('ready');
                        setMood('listening');
                    } else {
                        throw e;
                    }
                }
            } catch (e) {
                console.error("Command failed:", e);
                setMood('idle');
                setVoiceStatus('error');
                setErrorMessage(e.toString());
            }
        };

        const setupListeners = async () => {
            try {
                console.log("Setting up voice event listeners...");

                unlistenStt = await listen('stt-result', (event) => {
                    console.log("STT Result received:", event.payload);
                    processInput(event.payload);
                });

                unlistenVoiceReady = await listen('voice-ready', (event) => {
                    console.log("Voice ready:", event.payload);
                    setVoiceStatus(event.payload ? 'ready' : 'stopped');
                    if (event.payload) {
                        setMood('listening');
                    }
                });

                unlistenVoiceProcessing = await listen('voice-processing', (event) => {
                    console.log("Voice processing...");
                    setVoiceStatus('processing');
                    setMood('thinking');
                });

                unlistenVoiceError = await listen('voice-error', (event) => {
                    console.error("Voice error:", event.payload);
                    setVoiceStatus('error');
                    setErrorMessage(event.payload);
                    setMood('idle');
                });

                console.log("Voice listeners setup complete");
            } catch (e) {
                console.error("Failed to setup listeners:", e);
            }
        };

        setupListeners();

        // Cleanup
        return () => {
            console.log("Cleaning up voice listeners...");
            if (unlistenStt) unlistenStt();
            if (unlistenVoiceReady) unlistenVoiceReady();
            if (unlistenVoiceProcessing) unlistenVoiceProcessing();
            if (unlistenVoiceError) unlistenVoiceError();
            invoke('stop_listening').catch(() => { });
        };
    }, []);

    // Auto-start voice on mount
    useEffect(() => {
        const autoStart = async () => {
            try {
                console.log("Auto-starting voice recognition...");
                await invoke('start_listening');
            } catch (e) {
                console.error("Auto-start failed:", e);
                setVoiceStatus('error');
                setErrorMessage("Voice recognition unavailable: " + e);
            }
        };

        // Delay slightly to allow UI to render
        const timer = setTimeout(autoStart, 1000);
        return () => clearTimeout(timer);
    }, []);

    const getStatusColor = () => {
        switch (voiceStatus) {
            case 'ready': return 'text-emo-cyan';
            case 'listening': return 'text-green-400';
            case 'processing': return 'text-yellow-400';
            case 'error': return 'text-red-400';
            default: return 'text-white/40';
        }
    };

    const getStatusText = () => {
        switch (voiceStatus) {
            case 'ready': return 'Ready';
            case 'listening': return 'Listening...';
            case 'processing': return 'Processing...';
            case 'error': return 'Error';
            case 'starting': return 'Starting...';
            default: return 'Off';
        }
    };

    return (
        <motion.div
            className="w-full h-full relative flex items-start justify-center p-4 select-none"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
        >
            {/* Draggable Container */}
            <motion.div
                className={`flex flex-col items-center relative shadow-[0_10px_40px_-10px_rgba(0,0,0,0.8)] border border-white/5 ring-1 ring-white/5 overflow-hidden group transition-all duration-500 bg-neutral-900/95 backdrop-blur-2xl rounded-[28px]`}
                style={{
                    width: isExpanded ? 300 : 140,
                    height: isExpanded ? 400 : 140,
                }}
                data-tauri-drag-region
                onMouseEnter={() => setIsHovered(true)}
                onMouseLeave={() => setIsHovered(false)}
            >
                {/* Status Indicator */}
                <div className={`absolute top-2 left-3 flex items-center gap-1.5 z-30 transition-opacity duration-300 ${isHovered || voiceStatus !== 'stopped' ? 'opacity-100' : 'opacity-0'}`}>
                    <div className={`w-2 h-2 rounded-full ${voiceStatus === 'listening' ? 'animate-pulse bg-green-400' : voiceStatus === 'processing' ? 'bg-yellow-400' : voiceStatus === 'error' ? 'bg-red-400' : voiceStatus === 'ready' ? 'bg-emo-cyan' : 'bg-white/20'}`} />
                    <span className={`text-[10px] ${getStatusColor()}`}>{getStatusText()}</span>
                </div>

                {/* Error Message */}
                {errorMessage && (
                    <div className="absolute top-8 left-2 right-2 z-40">
                        <div className="bg-red-500/20 border border-red-500/30 rounded-lg px-2 py-1 text-[9px] text-red-200 text-center">
                            {errorMessage}
                        </div>
                    </div>
                )}

                {/* Header / Controls */}
                <div className={`absolute top-3 right-3 flex gap-1.5 z-30 transition-all duration-300 ${isExpanded ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}`}>
                    {isExpanded && (
                        <button
                            onClick={() => setShowSettings(s => !s)}
                            className={`p-1.5 rounded-full transition-colors backdrop-blur-sm ${showSettings ? 'bg-emo-cyan/20 text-emo-cyan' : 'bg-white/5 hover:bg-white/10 text-white/50 hover:text-white'}`}
                            title="Settings"
                        >
                            <Settings2 size={13} />
                        </button>
                    )}
                    <button
                        onClick={toggleExpand}
                        className="p-1.5 rounded-full bg-white/5 hover:bg-white/10 text-white/50 hover:text-white transition-colors backdrop-blur-sm"
                    >
                        {isExpanded ? <X size={14} /> : <Menu size={14} />}
                    </button>
                </div>

                {/* Eyes Container */}
                <motion.div
                    className={`pointer-events-none transform transition-all duration-500 z-10 w-full flex justify-center ${isExpanded ? 'mt-4 scale-75' : 'scale-90 mt-0 flex-1 items-center'}`}
                >
                    <Eyes mood={mood} />
                </motion.div>

                {/* Chat / Settings Interface */}
                <AnimatePresence mode="wait">
                    {isExpanded && (
                        <motion.div
                            className="w-full flex-1 overflow-hidden"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            exit={{ opacity: 0 }}
                        >
                            {showSettings
                                ? <Settings onClose={() => setShowSettings(false)} />
                                : <Chat onMoodChange={setMood} />
                            }
                        </motion.div>
                    )}
                </AnimatePresence>

                {/* Bottom Controls */}
                {!isExpanded && (
                    <div className="absolute bottom-3 flex gap-4 z-30 opacity-0 group-hover:opacity-100 transition-all duration-300 transform translate-y-2 group-hover:translate-y-0">
                        <button
                            className={`p-2 rounded-full transition-all duration-300 ${voiceStatus === 'listening' || voiceStatus === 'ready' ? 'bg-emo-cyan/20 text-emo-cyan shadow-[0_0_15px_rgba(0,217,255,0.3)]' : 'bg-white/5 hover:bg-white/10 text-white/40 hover:text-white'}`}
                            onClick={toggleListening}
                            title={voiceStatus === 'stopped' ? 'Start listening' : 'Stop listening'}
                        >
                            {voiceStatus === 'listening' || voiceStatus === 'ready' ?
                                <Disc size={18} className="animate-spin-slow" /> :
                                <Mic size={18} />
                            }
                        </button>
                        <button
                            className="p-2 rounded-full bg-white/5 hover:bg-white/10 text-white/40 hover:text-white transition-colors"
                            onClick={toggleExpand}
                        >
                            <Menu size={18} />
                        </button>
                    </div>
                )}

                {/* Status Glow Effect */}
                <div className={`absolute inset-0 rounded-[28px] transition-opacity duration-700 pointer-events-none z-0 ${voiceStatus === 'listening' ? 'opacity-100' : 'opacity-0'} bg-gradient-to-t from-emo-cyan/10 via-transparent to-transparent`} />

            </motion.div>
        </motion.div>
    );
};

export default EmoRobot;

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
    X, Cpu, Mic, Palette, Activity,
    Download, Trash2, ChevronRight,
    ToggleLeft, ToggleRight, Info
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

const Section = ({ title, icon: Icon, children }) => (
    <div className="mb-4">
        <div className="flex items-center gap-2 mb-2 px-1">
            <Icon size={12} className="text-emo-cyan/70" />
            <span className="text-[10px] font-bold uppercase tracking-widest text-white/40">{title}</span>
        </div>
        <div className="space-y-1.5">{children}</div>
    </div>
);

const Row = ({ label, children, desc }) => (
    <div className="flex items-center justify-between gap-4 bg-white/3 hover:bg-white/5 rounded-xl px-3 py-2.5 transition-colors">
        <div>
            <span className="text-[12px] text-white/80">{label}</span>
            {desc && <p className="text-[10px] text-white/30 mt-0.5">{desc}</p>}
        </div>
        <div className="shrink-0">{children}</div>
    </div>
);

const Toggle = ({ value, onChange }) => (
    <button onClick={() => onChange(!value)} className="transition-opacity hover:opacity-80">
        {value
            ? <ToggleRight size={20} className="text-emo-cyan" />
            : <ToggleLeft size={20} className="text-white/30" />
        }
    </button>
);

const StatusDot = ({ loaded }) => (
    <span className={`inline-block w-2 h-2 rounded-full ${loaded ? 'bg-green-400' : 'bg-white/20'}`} />
);

const Settings = ({ onClose }) => {
    const [tab, setTab] = useState('model');
    const [modelStatus, setModelStatus] = useState({ small_loaded: false, large_loaded: false });
    const [loadingSmall, setLoadingSmall] = useState(false);
    const [loadingLarge, setLoadingLarge] = useState(false);
    const [sysInfo, setSysInfo] = useState('');
    const [voiceContinuous, setVoiceContinuous] = useState(true);
    const [wakeWord, setWakeWord] = useState(true);
    const [ttsEnabled, setTtsEnabled] = useState(true);
    const [alwaysOnTop, setAlwaysOnTop] = useState(true);
    const [toast, setToast] = useState(null);

    const showToast = (msg, err = false) => {
        setToast({ msg, err });
        setTimeout(() => setToast(null), 2500);
    };

    useEffect(() => {
        const load = async () => {
            try {
                const status = await invoke('get_model_status');
                setModelStatus(status);
                const info = await invoke('get_system_status');
                setSysInfo(info);
            } catch (e) {
                console.error(e);
            }
        };
        load();

        // Auto-unload 1.5B model after idle timeout (5 min)
        const idlePoller = setInterval(async () => {
            try {
                const isIdle = await invoke('is_large_model_idle');
                if (isIdle) {
                    await invoke('unload_large_model');
                    setModelStatus(prev => ({ ...prev, large_loaded: false }));
                    showToast('1.5B model auto-unloaded (idle > 5 min)');
                }
            } catch (_) { }
        }, 60_000);

        return () => clearInterval(idlePoller);
    }, []);

    const handleLoadSmall = async () => {
        setLoadingSmall(true);
        try {
            const msg = await invoke('load_model');
            setModelStatus(prev => ({ ...prev, small_loaded: true }));
            showToast(msg);
        } catch (e) {
            showToast(String(e), true);
        } finally { setLoadingSmall(false); }
    };

    const handleLoadLarge = async () => {
        setLoadingLarge(true);
        try {
            const msg = await invoke('load_large_model');
            setModelStatus(prev => ({ ...prev, large_loaded: true }));
            showToast(msg);
        } catch (e) {
            showToast(String(e), true);
        } finally { setLoadingLarge(false); }
    };

    const handleUnloadLarge = async () => {
        try {
            const msg = await invoke('unload_large_model');
            setModelStatus(prev => ({ ...prev, large_loaded: false }));
            showToast(msg);
        } catch (e) {
            showToast(String(e), true);
        }
    };

    const tabs = [
        { id: 'model', label: 'AI' },
        { id: 'voice', label: 'Voice' },
        { id: 'appearance', label: 'Display' },
        { id: 'system', label: 'System' },
    ];

    return (
        <motion.div
            className="w-full h-full flex flex-col bg-neutral-950/95 backdrop-blur-2xl rounded-[22px] overflow-hidden"
            initial={{ opacity: 0, scale: 0.97 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.97 }}
        >
            {/* Header */}
            <div className="flex items-center justify-between px-4 pt-4 pb-2 border-b border-white/5">
                <span className="text-[13px] font-semibold text-white/80 tracking-wide">Settings</span>
                <button onClick={onClose} className="p-1.5 rounded-full hover:bg-white/10 text-white/40 hover:text-white transition-colors">
                    <X size={13} />
                </button>
            </div>

            {/* Tab Bar */}
            <div className="flex gap-1 px-3 pt-2 pb-1">
                {tabs.map(t => (
                    <button
                        key={t.id}
                        onClick={() => setTab(t.id)}
                        className={`flex-1 py-1.5 rounded-lg text-[10px] font-semibold tracking-wide transition-all ${tab === t.id
                            ? 'bg-emo-cyan/20 text-emo-cyan border border-emo-cyan/30'
                            : 'text-white/30 hover:text-white/60 hover:bg-white/5'
                            }`}
                    >
                        {t.label}
                    </button>
                ))}
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto px-3 py-3 custom-scrollbar">
                <AnimatePresence mode="wait">
                    {tab === 'model' && (
                        <motion.div key="model" initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -10 }}>
                            <Section title="AI Models" icon={Cpu}>
                                <Row
                                    label="Qwen 2.5-0.5B"
                                    desc="Fast • ~400 MB • Casual chat"
                                >
                                    <div className="flex items-center gap-2">
                                        <StatusDot loaded={modelStatus.small_loaded} />
                                        <button
                                            onClick={handleLoadSmall}
                                            disabled={loadingSmall || modelStatus.small_loaded}
                                            className="text-[10px] px-2 py-1 rounded-lg bg-emo-cyan/15 text-emo-cyan border border-emo-cyan/25 hover:bg-emo-cyan/25 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                        >
                                            {loadingSmall ? 'Loading…' : modelStatus.small_loaded ? 'Loaded' : 'Load'}
                                        </button>
                                    </div>
                                </Row>

                                <Row
                                    label="Qwen 2.5-1.5B"
                                    desc="Smarter • ~1.2 GB • Complex tasks"
                                >
                                    <div className="flex items-center gap-2">
                                        <StatusDot loaded={modelStatus.large_loaded} />
                                        {modelStatus.large_loaded ? (
                                            <button
                                                onClick={handleUnloadLarge}
                                                className="text-[10px] px-2 py-1 rounded-lg bg-red-500/15 text-red-400 border border-red-500/25 hover:bg-red-500/25 transition-colors"
                                            >
                                                <Trash2 size={10} />
                                            </button>
                                        ) : (
                                            <button
                                                onClick={handleLoadLarge}
                                                disabled={loadingLarge}
                                                className="text-[10px] px-2 py-1 rounded-lg bg-blue-500/15 text-blue-400 border border-blue-500/25 hover:bg-blue-500/25 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                            >
                                                {loadingLarge ? 'Loading…' : 'Load'}
                                            </button>
                                        )}
                                    </div>
                                </Row>
                            </Section>

                            <Section title="Routing" icon={ChevronRight}>
                                <Row label="Auto-select model" desc="Route to 1.5B for complex tasks automatically">
                                    <span className="text-[10px] text-emo-cyan/70 font-mono">Auto</span>
                                </Row>
                                <div className="mt-2 p-2 rounded-xl bg-white/3 border border-white/5">
                                    <p className="text-[10px] text-white/30 leading-relaxed">
                                        <Info size={9} className="inline mr-1 text-white/20" />
                                        Simple greetings → 0.5B · File/app tasks, long prompts → 1.5B (if loaded, otherwise falls back to 0.5B)
                                    </p>
                                </div>
                            </Section>
                        </motion.div>
                    )}

                    {tab === 'voice' && (
                        <motion.div key="voice" initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -10 }}>
                            <Section title="Listening" icon={Mic}>
                                <Row label="Continuous listening" desc="Keep microphone active after a command">
                                    <Toggle value={voiceContinuous} onChange={setVoiceContinuous} />
                                </Row>
                                <Row label="Wake word required" desc={'Say "Hey Emo" before each command'}>
                                    <Toggle value={wakeWord} onChange={setWakeWord} />
                                </Row>
                            </Section>
                            <Section title="Speech Output" icon={Activity}>
                                <Row label="Text-to-Speech" desc="Speak responses using Piper TTS">
                                    <Toggle value={ttsEnabled} onChange={setTtsEnabled} />
                                </Row>
                                <Row label="Voice speed" desc="Cartoon voice profile (1.1×)">
                                    <span className="text-[11px] text-emo-cyan/70 font-mono">1.1×</span>
                                </Row>
                                <Row label="Pitch" desc="Higher pitch is more expressive">
                                    <span className="text-[11px] text-emo-cyan/70 font-mono">+15%</span>
                                </Row>
                            </Section>
                        </motion.div>
                    )}

                    {tab === 'appearance' && (
                        <motion.div key="appearance" initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -10 }}>
                            <Section title="Window" icon={Palette}>
                                <Row label="Always on top" desc="Keep Emo above other windows">
                                    <Toggle value={alwaysOnTop} onChange={setAlwaysOnTop} />
                                </Row>
                                <Row label="Widget size" desc="Compact (140px) / Expanded (300px)">
                                    <span className="text-[11px] text-emo-cyan/70 font-mono">Auto</span>
                                </Row>
                            </Section>
                            <Section title="Eyes" icon={Activity}>
                                <Row label="Animation FPS" desc="60 FPS active · 30 FPS idle">
                                    <span className="text-[11px] text-emo-cyan/70 font-mono">Adaptive</span>
                                </Row>
                                <Row label="Eye color" desc="Default: Cyan #00d9ff">
                                    <span
                                        className="inline-block w-4 h-4 rounded-full border border-white/20"
                                        style={{ background: '#00d9ff' }}
                                    />
                                </Row>
                            </Section>
                        </motion.div>
                    )}

                    {tab === 'system' && (
                        <motion.div key="system" initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -10 }}>
                            <Section title="Performance" icon={Activity}>
                                <div className="bg-black/30 rounded-xl px-3 py-2.5 font-mono text-[10px] text-green-400/80 whitespace-pre-line leading-relaxed border border-white/5">
                                    {sysInfo || 'Loading system info…'}
                                </div>
                            </Section>
                            <Section title="Data" icon={Cpu}>
                                <Row label="Conversation history" desc="Stored locally in SQLite">
                                    <span className="text-[10px] text-white/30">7 days</span>
                                </Row>
                                <Row label="Data location" desc="All data stays on your device">
                                    <span className="text-[10px] text-emo-cyan/50 font-mono">%APPDATA%</span>
                                </Row>
                                <Row label="Telemetry" desc="Zero telemetry, fully offline">
                                    <span className="text-[10px] text-green-400/70">None ✓</span>
                                </Row>
                            </Section>
                            <Section title="About" icon={Info}>
                                <div className="bg-white/3 rounded-xl px-3 py-2.5 border border-white/5 text-center">
                                    <p className="text-[11px] text-white/60 font-semibold">Emo Robot Companion</p>
                                    <p className="text-[10px] text-white/30 mt-0.5">v1.0 • Local AI • MIT License</p>
                                    <p className="text-[10px] text-white/20 mt-1">Tauri 2 + Qwen 2.5 + Candle-rs</p>
                                </div>
                            </Section>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Toast notification */}
            <AnimatePresence>
                {toast && (
                    <motion.div
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: 10 }}
                        className={`absolute bottom-4 left-3 right-3 text-center py-2 px-3 rounded-xl text-[11px] font-medium border ${toast.err
                            ? 'bg-red-500/20 text-red-300 border-red-500/30'
                            : 'bg-green-500/20 text-green-300 border-green-500/30'
                            }`}
                    >
                        {toast.msg}
                    </motion.div>
                )}
            </AnimatePresence>
        </motion.div>
    );
};

export default Settings;

import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Download, CheckCircle, AlertCircle, Loader2, Cpu, Mic, Volume2, Sparkles } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const MODEL_ICONS = {
    'qwen-0.5b': Cpu,
    'whisper-tiny': Mic,
    'piper-tts': Volume2,
};

const MODEL_COLORS = {
    'qwen-0.5b': { gradient: 'from-cyan-500 to-blue-600', glow: 'rgba(0,217,255,0.3)', text: 'text-cyan-400' },
    'whisper-tiny': { gradient: 'from-purple-500 to-pink-500', glow: 'rgba(168,85,247,0.3)', text: 'text-purple-400' },
    'piper-tts': { gradient: 'from-amber-500 to-orange-500', glow: 'rgba(245,158,11,0.3)', text: 'text-amber-400' },
};

const SetupScreen = ({ onComplete }) => {
    const [stage, setStage] = useState('welcome'); // welcome | downloading | complete | error
    const [models, setModels] = useState([]);
    const [modelProgress, setModelProgress] = useState({});
    const [error, setError] = useState(null);
    const [overallProgress, setOverallProgress] = useState(0);
    const unlistenRef = useRef(null);

    // Load model list
    useEffect(() => {
        const loadModels = async () => {
            try {
                const list = await invoke('get_available_models');
                setModels(list);
                // Initialize progress
                const initial = {};
                list.forEach(m => {
                    initial[m.id] = { status: 'pending', message: 'Waiting...', filesDone: 0, filesTotal: 1 };
                });
                setModelProgress(initial);
            } catch (e) {
                console.error('Failed to get model list:', e);
            }
        };
        loadModels();
    }, []);

    // Listen for download progress events
    useEffect(() => {
        let unlisten = null;
        const setup = async () => {
            unlisten = await listen('model-download-progress', (event) => {
                const p = event.payload;
                setModelProgress(prev => ({
                    ...prev,
                    [p.model_id]: {
                        status: p.status,
                        message: p.message,
                        fileName: p.file_name,
                        filesDone: p.files_done,
                        filesTotal: p.files_total,
                    }
                }));
            });
            unlistenRef.current = unlisten;
        };
        setup();

        return () => {
            if (unlistenRef.current) unlistenRef.current();
        };
    }, []);

    // Calculate overall progress
    useEffect(() => {
        const entries = Object.values(modelProgress);
        if (entries.length === 0) return;
        const completed = entries.filter(e => e.status === 'complete').length;
        setOverallProgress(Math.round((completed / entries.length) * 100));
    }, [modelProgress]);

    const handleStartDownload = async () => {
        setStage('downloading');
        setError(null);
        try {
            await invoke('download_models');
            setStage('complete');
        } catch (e) {
            console.error('Download failed:', e);
            setError(String(e));
            setStage('error');
        }
    };

    const handleRetry = () => {
        // Reset states
        const initial = {};
        models.forEach(m => {
            initial[m.id] = { status: 'pending', message: 'Waiting...', filesDone: 0, filesTotal: 1 };
        });
        setModelProgress(initial);
        setError(null);
        handleStartDownload();
    };

    return (
        <div className="w-full h-full flex flex-col items-center justify-center bg-neutral-950 relative overflow-hidden">
            {/* Animated background grid */}
            <div className="absolute inset-0 opacity-[0.03]"
                style={{
                    backgroundImage: `radial-gradient(circle at 1px 1px, white 1px, transparent 0)`,
                    backgroundSize: '24px 24px',
                }} />

            {/* Ambient glow */}
            <motion.div
                className="absolute w-[300px] h-[300px] rounded-full blur-[100px] opacity-20"
                style={{
                    background: stage === 'complete'
                        ? 'radial-gradient(circle, rgba(34,197,94,0.6), transparent)'
                        : stage === 'error'
                            ? 'radial-gradient(circle, rgba(239,68,68,0.6), transparent)'
                            : 'radial-gradient(circle, rgba(0,217,255,0.6), transparent)',
                }}
                animate={{
                    scale: [1, 1.2, 1],
                    opacity: [0.15, 0.25, 0.15],
                }}
                transition={{ duration: 4, repeat: Infinity, ease: 'easeInOut' }}
            />

            <AnimatePresence mode="wait">
                {/* ── Welcome Stage ── */}
                {stage === 'welcome' && (
                    <motion.div
                        key="welcome"
                        className="flex flex-col items-center gap-6 px-6 z-10 max-w-[280px]"
                        initial={{ opacity: 0, y: 20 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -20 }}
                        transition={{ duration: 0.4 }}
                    >
                        {/* Logo */}
                        <motion.div
                            className="relative"
                            animate={{ y: [0, -6, 0] }}
                            transition={{ duration: 3, repeat: Infinity, ease: 'easeInOut' }}
                        >
                            <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-cyan-500 to-blue-600 flex items-center justify-center shadow-lg shadow-cyan-500/20">
                                <Sparkles size={28} className="text-white" />
                            </div>
                            <div className="absolute -bottom-1 left-1/2 -translate-x-1/2 w-10 h-2 bg-cyan-500/10 rounded-full blur-sm" />
                        </motion.div>

                        <div className="text-center">
                            <h1 className="text-lg font-bold text-white tracking-tight">Welcome to Emo</h1>
                            <p className="text-[11px] text-white/40 mt-1.5 leading-relaxed">
                                Your AI companion needs to download a few models before it can think, listen, and speak.
                            </p>
                        </div>

                        {/* Model List Preview */}
                        <div className="w-full space-y-2">
                            {models.map((model, i) => {
                                const Icon = MODEL_ICONS[model.id] || Cpu;
                                const colors = MODEL_COLORS[model.id] || MODEL_COLORS['qwen-0.5b'];
                                return (
                                    <motion.div
                                        key={model.id}
                                        initial={{ opacity: 0, x: -10 }}
                                        animate={{ opacity: 1, x: 0 }}
                                        transition={{ delay: 0.1 * (i + 1) }}
                                        className="flex items-center gap-3 bg-white/[0.03] border border-white/[0.06] rounded-xl px-3 py-2.5"
                                    >
                                        <div className={`w-8 h-8 rounded-lg bg-gradient-to-br ${colors.gradient} flex items-center justify-center shrink-0 shadow-sm`}>
                                            <Icon size={14} className="text-white" />
                                        </div>
                                        <div className="flex-1 min-w-0">
                                            <p className="text-[11px] font-semibold text-white/80">{model.name}</p>
                                            <p className="text-[9px] text-white/30 truncate">{model.description}</p>
                                        </div>
                                        <span className="text-[9px] text-white/20 font-mono shrink-0">{model.size_label}</span>
                                    </motion.div>
                                );
                            })}
                        </div>

                        {/* Download Button */}
                        <motion.button
                            onClick={handleStartDownload}
                            className="w-full py-3 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 text-white text-[12px] font-semibold flex items-center justify-center gap-2 shadow-lg shadow-cyan-500/20 hover:shadow-cyan-500/40 transition-shadow"
                            whileHover={{ scale: 1.02 }}
                            whileTap={{ scale: 0.98 }}
                        >
                            <Download size={14} />
                            Download All Models
                        </motion.button>

                        <p className="text-[9px] text-white/20 text-center">
                            ~540 MB total • Downloads from HuggingFace
                        </p>
                    </motion.div>
                )}

                {/* ── Downloading Stage ── */}
                {stage === 'downloading' && (
                    <motion.div
                        key="downloading"
                        className="flex flex-col items-center gap-5 px-6 z-10 w-full max-w-[280px]"
                        initial={{ opacity: 0, y: 20 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -20 }}
                        transition={{ duration: 0.4 }}
                    >
                        {/* Pulsing Loader */}
                        <motion.div
                            className="relative"
                            animate={{ rotate: 360 }}
                            transition={{ duration: 8, repeat: Infinity, ease: 'linear' }}
                        >
                            <div className="w-14 h-14 rounded-full border-2 border-cyan-500/20 flex items-center justify-center">
                                <Loader2 size={24} className="text-cyan-400 animate-spin" />
                            </div>
                        </motion.div>

                        <div className="text-center">
                            <h2 className="text-[14px] font-bold text-white">Downloading Models</h2>
                            <p className="text-[10px] text-white/30 mt-1">This may take a few minutes...</p>
                        </div>

                        {/* Model Progress Cards */}
                        <div className="w-full space-y-2">
                            {models.map((model) => {
                                const Icon = MODEL_ICONS[model.id] || Cpu;
                                const colors = MODEL_COLORS[model.id] || MODEL_COLORS['qwen-0.5b'];
                                const prog = modelProgress[model.id] || { status: 'pending', message: 'Waiting...' };
                                const isActive = prog.status === 'downloading';
                                const isDone = prog.status === 'complete';
                                const isError = prog.status === 'error';

                                return (
                                    <motion.div
                                        key={model.id}
                                        className={`relative overflow-hidden rounded-xl border transition-all duration-500 ${isDone
                                            ? 'bg-green-500/[0.06] border-green-500/20'
                                            : isActive
                                                ? 'bg-white/[0.04] border-cyan-500/30'
                                                : isError
                                                    ? 'bg-red-500/[0.06] border-red-500/20'
                                                    : 'bg-white/[0.02] border-white/[0.06]'
                                            }`}
                                        layout
                                    >
                                        <div className="flex items-center gap-3 px-3 py-2.5 relative z-10">
                                            <div className={`w-7 h-7 rounded-lg flex items-center justify-center shrink-0 ${isDone
                                                ? 'bg-green-500/20'
                                                : `bg-gradient-to-br ${colors.gradient} shadow-sm`
                                                }`}>
                                                {isDone
                                                    ? <CheckCircle size={13} className="text-green-400" />
                                                    : isError
                                                        ? <AlertCircle size={13} className="text-red-400" />
                                                        : <Icon size={13} className="text-white" />
                                                }
                                            </div>

                                            <div className="flex-1 min-w-0">
                                                <div className="flex items-center gap-2 justify-between">
                                                    <p className="text-[11px] font-semibold text-white/80">{model.name}</p>
                                                    {isDone && <span className="text-[8px] text-green-400 font-bold uppercase">Ready</span>}
                                                    {isActive && <Loader2 size={10} className="text-cyan-400 animate-spin" />}
                                                </div>
                                                <p className={`text-[9px] mt-0.5 truncate ${isDone ? 'text-green-400/50' : isActive ? 'text-cyan-400/60' : 'text-white/30'}`}>
                                                    {prog.message}
                                                </p>
                                            </div>
                                        </div>

                                        {/* Animated progress shimmer for active download */}
                                        {isActive && (
                                            <motion.div
                                                className="absolute bottom-0 left-0 h-[2px] bg-gradient-to-r from-transparent via-cyan-400 to-transparent"
                                                animate={{ x: ['-100%', '200%'] }}
                                                transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}
                                                style={{ width: '50%' }}
                                            />
                                        )}
                                    </motion.div>
                                );
                            })}
                        </div>

                        {/* Overall progress bar */}
                        <div className="w-full">
                            <div className="flex justify-between mb-1">
                                <span className="text-[9px] text-white/30">Overall Progress</span>
                                <span className="text-[9px] text-cyan-400/70 font-mono">{overallProgress}%</span>
                            </div>
                            <div className="w-full h-1.5 bg-white/5 rounded-full overflow-hidden">
                                <motion.div
                                    className="h-full bg-gradient-to-r from-cyan-500 to-blue-500 rounded-full"
                                    initial={{ width: '0%' }}
                                    animate={{ width: `${overallProgress}%` }}
                                    transition={{ duration: 0.5, ease: 'easeOut' }}
                                />
                            </div>
                        </div>
                    </motion.div>
                )}

                {/* ── Complete Stage ── */}
                {stage === 'complete' && (
                    <motion.div
                        key="complete"
                        className="flex flex-col items-center gap-5 px-6 z-10 max-w-[280px]"
                        initial={{ opacity: 0, scale: 0.9 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.9 }}
                        transition={{ duration: 0.5, type: 'spring' }}
                    >
                        <motion.div
                            initial={{ scale: 0 }}
                            animate={{ scale: 1 }}
                            transition={{ delay: 0.2, type: 'spring', stiffness: 200 }}
                        >
                            <div className="w-16 h-16 rounded-full bg-green-500/20 flex items-center justify-center border border-green-500/30">
                                <CheckCircle size={32} className="text-green-400" />
                            </div>
                        </motion.div>

                        <div className="text-center">
                            <h2 className="text-lg font-bold text-white">All Set!</h2>
                            <p className="text-[11px] text-white/40 mt-1">
                                All AI models are downloaded and ready. Emo is now fully operational!
                            </p>
                        </div>

                        <motion.button
                            onClick={onComplete}
                            className="w-full py-3 rounded-xl bg-gradient-to-r from-green-500 to-emerald-600 text-white text-[12px] font-semibold flex items-center justify-center gap-2 shadow-lg shadow-green-500/20 hover:shadow-green-500/40 transition-shadow"
                            whileHover={{ scale: 1.02 }}
                            whileTap={{ scale: 0.98 }}
                        >
                            <Sparkles size={14} />
                            Launch Emo
                        </motion.button>
                    </motion.div>
                )}

                {/* ── Error Stage ── */}
                {stage === 'error' && (
                    <motion.div
                        key="error"
                        className="flex flex-col items-center gap-5 px-6 z-10 max-w-[280px]"
                        initial={{ opacity: 0, y: 20 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -20 }}
                    >
                        <div className="w-14 h-14 rounded-full bg-red-500/20 flex items-center justify-center border border-red-500/30">
                            <AlertCircle size={28} className="text-red-400" />
                        </div>

                        <div className="text-center">
                            <h2 className="text-[14px] font-bold text-white">Download Failed</h2>
                            <p className="text-[10px] text-white/40 mt-1 leading-relaxed">
                                Please check your internet connection and try again.
                            </p>
                            {error && (
                                <div className="mt-2 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2 max-h-[60px] overflow-y-auto">
                                    <p className="text-[9px] text-red-300/70 font-mono text-left break-all">{error}</p>
                                </div>
                            )}
                        </div>

                        <motion.button
                            onClick={handleRetry}
                            className="w-full py-3 rounded-xl bg-gradient-to-r from-red-500 to-rose-600 text-white text-[12px] font-semibold flex items-center justify-center gap-2 shadow-lg shadow-red-500/20"
                            whileHover={{ scale: 1.02 }}
                            whileTap={{ scale: 0.98 }}
                        >
                            <Download size={14} />
                            Retry Download
                        </motion.button>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default SetupScreen;

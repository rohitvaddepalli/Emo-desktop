import { motion } from 'framer-motion';
import { useState, useEffect } from 'react';

const Eye = ({ mood }) => {
    const [blink, setBlink] = useState(false);

    useEffect(() => {
        const blinkInterval = setInterval(() => {
            // Random blink timing between 3-6 seconds
            const nextBlink = Math.random() * 3000 + 3000;
            setTimeout(() => {
                setBlink(true);
                setTimeout(() => setBlink(false), 150); // Blink duration
            }, nextBlink);
        }, 4000); // Check/reset interval

        return () => clearInterval(blinkInterval);
    }, []);

    const normalizedMood = (mood === 'listening-wake' || mood === 'listening-active') ? 'listening' : mood;

    const variants = {
        idle: { scaleY: blink ? 0.1 : 1, scaleX: 1, backgroundColor: '#00d9ff' },
        happy: { scaleY: 0.5, scaleX: 1.2, borderRadius: '50% 50% 0 0', backgroundColor: '#00d9ff' }, // Upward curve
        thinking: { scale: 0.8, rotate: [0, 180, 360], transition: { repeat: Infinity, duration: 2 } },
        listening: { scale: [1, 1.2, 1], boxShadow: "0px 0px 20px #00d9ff", transition: { repeat: Infinity, duration: 1.5 } },
    };

    return (
        <motion.div
            className="w-12 h-16 rounded-full bg-emo-cyan shadow-[0_0_15px_rgba(0,217,255,0.6)]"
            animate={normalizedMood === 'idle' && blink ? { scaleY: 0.1 } : normalizedMood}
            variants={variants}
            transition={{ type: "spring", stiffness: 300, damping: 20 }}
        />
    );
};

const Eyes = ({ mood = 'idle' }) => {
    return (
        <div className="flex justify-center gap-6 items-center w-full h-32 relative">
            <Eye mood={mood} />
            <Eye mood={mood} />
        </div>
    );
};

export default Eyes;

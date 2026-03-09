import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import EmoRobot from './components/EmoRobot';
import SetupScreen from './components/SetupScreen';

function App() {
  const [appReady, setAppReady] = useState(null); // null = checking, true = ready, false = needs setup

  useEffect(() => {
    const checkModels = async () => {
      try {
        const result = await invoke('check_models_downloaded');
        setAppReady(result.all_ready);
      } catch (e) {
        console.error('Failed to check model status:', e);
        // If check fails, show setup screen to let user download
        setAppReady(false);
      }
    };
    checkModels();
  }, []);

  // Loading state while checking
  if (appReady === null) {
    return (
      <div className="w-screen h-screen flex items-center justify-center bg-neutral-950">
        <div className="w-6 h-6 border-2 border-cyan-500/30 border-t-cyan-500 rounded-full animate-spin" />
      </div>
    );
  }

  // Setup required — models not downloaded
  if (!appReady) {
    return (
      <div className="w-screen h-screen bg-neutral-950">
        <SetupScreen onComplete={() => setAppReady(true)} />
      </div>
    );
  }

  // Main app
  return (
    <div className="w-screen h-screen flex items-center justify-center relative overflow-hidden bg-transparent">
      <EmoRobot />
    </div>
  );
}

export default App;

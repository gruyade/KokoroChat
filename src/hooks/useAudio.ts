import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';

/** tts:audio イベントペイロード */
interface TTSAudioEvent {
  data: string; // Base64エンコードされた音声データ
}

/**
 * TTS音声再生制御Hook
 * - tts:audio イベントリスナー
 * - Web Audio APIによる音声再生
 */
export function useAudio() {
  const [isPlaying, setIsPlaying] = useState(false);
  const audioContextRef = useRef<AudioContext | null>(null);
  const sourceNodeRef = useRef<AudioBufferSourceNode | null>(null);

  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<TTSAudioEvent>('tts:audio', async (event) => {
        await playAudio(event.payload.data);
      });

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then((fn) => {
      cleanup = fn;
    });

    return () => {
      cleanup?.();
      stop();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const playAudio = async (base64Data: string) => {
    stop();

    if (!audioContextRef.current) {
      audioContextRef.current = new AudioContext();
    }

    const audioContext = audioContextRef.current;
    const binaryString = atob(base64Data);
    const bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }

    const audioBuffer = await audioContext.decodeAudioData(bytes.buffer);
    const source = audioContext.createBufferSource();
    source.buffer = audioBuffer;
    source.connect(audioContext.destination);

    source.onended = () => {
      setIsPlaying(false);
      sourceNodeRef.current = null;
    };

    sourceNodeRef.current = source;
    setIsPlaying(true);
    source.start();
  };

  const stop = () => {
    if (sourceNodeRef.current) {
      try {
        sourceNodeRef.current.stop();
      } catch {
        // 既に停止済みの場合は無視
      }
      sourceNodeRef.current = null;
    }
    setIsPlaying(false);
  };

  return { isPlaying, stop };
}

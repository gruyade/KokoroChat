import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { create } from 'zustand';
import type { TTSCompleteEvent } from '../types';

const VOLUME_STORAGE_KEY = 'tts-volume';

/** TTS音声状態のグローバルストア */
interface AudioState {
  isPlaying: boolean;
  volume: number;
  setPlaying: (playing: boolean) => void;
  setVolume: (volume: number) => void;
}

export const useAudioStore = create<AudioState>((set) => ({
  isPlaying: false,
  volume: (() => {
    try {
      const saved = localStorage.getItem(VOLUME_STORAGE_KEY);
      return saved ? parseFloat(saved) : 1.0;
    } catch {
      return 1.0;
    }
  })(),
  setPlaying: (playing) => set({ isPlaying: playing }),
  setVolume: (volume) => {
    const clamped = Math.max(0, Math.min(1, volume));
    set({ volume: clamped });
    localStorage.setItem(VOLUME_STORAGE_KEY, clamped.toString());
  },
}));

/**
 * TTS音声再生制御Hook
 * - tts:complete イベントリスナー
 * - Web Audio APIによる音声再生
 * - GainNodeによるボリュームコントロール
 *
 * このhookはアプリ内で1箇所のみマウントすること
 */
export function useAudio() {
  const audioContextRef = useRef<AudioContext | null>(null);
  const sourceNodeRef = useRef<AudioBufferSourceNode | null>(null);
  const gainNodeRef = useRef<GainNode | null>(null);

  // ストアのvolumeが変わったらGainNodeに反映
  useEffect(() => {
    return useAudioStore.subscribe((state) => {
      if (gainNodeRef.current) {
        gainNodeRef.current.gain.value = state.volume;
      }
    });
  }, []);

  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<TTSCompleteEvent>('tts:complete', (event) => {
        const { audio } = event.payload;
        console.log('[useAudio] tts:complete received, audio length:', audio?.length ?? 0);
        if (audio) {
          playAudio(audio).catch((e) => console.error('[useAudio] listener error:', e));
        }
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
    try {
      console.log('[useAudio] playAudio called, data length:', base64Data.length);
      stop();

      if (!audioContextRef.current) {
        audioContextRef.current = new AudioContext();
        console.log('[useAudio] Created new AudioContext, state:', audioContextRef.current.state);
      }

      const audioContext = audioContextRef.current;

      if (audioContext.state === 'suspended') {
        await audioContext.resume();
      }

      // GainNode作成（ボリュームコントロール用）
      if (!gainNodeRef.current) {
        gainNodeRef.current = audioContext.createGain();
        gainNodeRef.current.connect(audioContext.destination);
      }
      gainNodeRef.current.gain.value = useAudioStore.getState().volume;

      const binaryString = atob(base64Data);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }

      const audioBuffer = await audioContext.decodeAudioData(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
      console.log('[useAudio] Decoded audio buffer, duration:', audioBuffer.duration, 'sampleRate:', audioBuffer.sampleRate);
      const source = audioContext.createBufferSource();
      source.buffer = audioBuffer;
      source.connect(gainNodeRef.current);

      source.onended = () => {
        useAudioStore.getState().setPlaying(false);
        sourceNodeRef.current = null;
      };

      sourceNodeRef.current = source;
      useAudioStore.getState().setPlaying(true);
      source.start();
    } catch (e) {
      console.error('[useAudio] playAudio failed:', e);
      useAudioStore.getState().setPlaying(false);
    }
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
    useAudioStore.getState().setPlaying(false);
  };
}

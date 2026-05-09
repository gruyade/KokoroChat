import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AvatarImageProps {
  avatarPath: string;
  alt?: string;
  className?: string;
}

/**
 * アバター画像コンポーネント
 * avatar_pathからBase64で読み込んで表示する
 */
export function AvatarImage({ avatarPath, alt = 'アバター', className = '' }: AvatarImageProps) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<string>('read_avatar', { avatarPath })
      .then((base64) => {
        if (!cancelled) setSrc(`data:image/png;base64,${base64}`);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });
    return () => { cancelled = true; };
  }, [avatarPath]);

  if (!src) return null;

  return <img src={src} alt={alt} className={className} />;
}

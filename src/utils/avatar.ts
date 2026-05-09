import { invoke, convertFileSrc } from '@tauri-apps/api/core';

/**
 * アバター画像のURLを取得する
 * convertFileSrcが動作しない場合はBase64データURLにフォールバック
 */
export async function getAvatarUrl(avatarPath: string): Promise<string> {
  try {
    // まずread_avatarでBase64読み込み（確実に動作する）
    const base64 = await invoke<string>('read_avatar', { avatarPath });
    return `data:image/png;base64,${base64}`;
  } catch {
    // フォールバック: convertFileSrc
    return convertFileSrc(avatarPath);
  }
}

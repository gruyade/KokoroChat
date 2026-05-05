/** ファイル添付の種別 */
export type AttachmentType = 'text' | 'pdf' | 'image';

/** 添付ファイル情報 */
export interface Attachment {
  id: string;
  file_name: string;
  file_path: string;
  attachment_type: AttachmentType;
  size_bytes: number;
  /** テキスト/PDF抽出結果 */
  extracted_text?: string;
  /** 画像のBase64エンコード */
  base64_data?: string;
}

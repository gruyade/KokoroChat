import { FileText, Image, File, X } from 'lucide-react';
import type { Attachment } from '../../types';

interface AttachmentPreviewProps {
  attachment: Attachment;
  onRemove?: () => void;
}

function getTypeIcon(type: Attachment['attachment_type']) {
  switch (type) {
    case 'text':
      return FileText;
    case 'pdf':
      return File;
    case 'image':
      return Image;
    default:
      return File;
  }
}

export function AttachmentPreview({ attachment, onRemove }: AttachmentPreviewProps) {
  const Icon = getTypeIcon(attachment.attachment_type);

  return (
    <div className="flex items-center gap-2 rounded-md border border-border bg-muted/50 px-2 py-1.5 text-sm">
      <Icon className="h-4 w-4 shrink-0 text-muted-foreground" />
      <span className="truncate max-w-[120px] text-foreground">{attachment.file_name}</span>
      {onRemove && (
        <button
          onClick={onRemove}
          className="ml-auto p-0.5 rounded hover:bg-destructive/20 text-muted-foreground hover:text-destructive"
          aria-label="添付ファイルを削除"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}

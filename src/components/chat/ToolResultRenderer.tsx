import { useMemo } from 'react';
import { getWidget, JsonViewer } from './widgets';

interface ToolResultRendererProps {
  content: string;
}

/** パース結果の型 */
type ParsedContent =
  | { kind: 'widget'; type: string; data: unknown }
  | { kind: 'text'; text: string };

/**
 * ChatWidget タグの正規表現
 * 例: <ChatWidget type="chart" data='{"labels":["A","B"]}' />
 */
const CHAT_WIDGET_REGEX = /<ChatWidget\s+type=["']([^"']+)["']\s+data=["'](.+?)["']\s*\/>/gs;

/**
 * ツール結果コンテンツを解析し、カスタムウィジェットまたはプレーンテキストとして描画する。
 *
 * 検出ロジック:
 * 1. JSON全体が {"widget": "...", "data": {...}} 形式 → ウィジェット描画
 * 2. <ChatWidget type="..." data='...' /> タグを含む → タグ部分をウィジェット描画、残りはテキスト
 * 3. それ以外 → プレーンテキスト表示
 */
export function ToolResultRenderer({ content }: ToolResultRendererProps) {
  const parsed = useMemo(() => parseToolResult(content), [content]);

  return (
    <div className="flex flex-col gap-1.5">
      {parsed.map((segment, i) => (
        <ToolResultSegment key={i} segment={segment} />
      ))}
    </div>
  );
}

function ToolResultSegment({ segment }: { segment: ParsedContent }) {
  if (segment.kind === 'text') {
    return (
      <pre className="text-[11px] text-muted-foreground whitespace-pre-wrap break-all font-mono bg-muted/40 rounded p-1.5 overflow-x-auto max-h-[200px] overflow-y-auto">
        {segment.text}
      </pre>
    );
  }

  // ウィジェット描画
  const Widget = getWidget(segment.type);
  if (Widget) {
    return <Widget data={segment.data} />;
  }

  // 未知のウィジェットタイプ → JSONフォールバック
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[10px] text-muted-foreground/70">
        未対応ウィジェット: {segment.type}
      </span>
      <JsonViewer data={segment.data} />
    </div>
  );
}

/**
 * ツール結果文字列をパースし、描画セグメントの配列を返す。
 */
function parseToolResult(content: string): ParsedContent[] {
  // 1. JSON全体がウィジェット形式かチェック
  const trimmed = content.trim();
  if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
    try {
      const json = JSON.parse(trimmed);
      if (json && typeof json === 'object' && 'widget' in json && 'data' in json) {
        return [{ kind: 'widget', type: json.widget as string, data: json.data }];
      }
      // widget フィールドがないJSON → JSONビューアで表示
      return [{ kind: 'widget', type: 'json', data: json }];
    } catch {
      // JSONパース失敗 → テキストとして続行
    }
  }

  // 2. <ChatWidget .../> タグの検出
  const segments: ParsedContent[] = [];
  let lastIndex = 0;

  // RegExpのlastIndexをリセット
  CHAT_WIDGET_REGEX.lastIndex = 0;

  let match: RegExpExecArray | null;
  while ((match = CHAT_WIDGET_REGEX.exec(content)) !== null) {
    // タグ前のテキスト
    if (match.index > lastIndex) {
      const textBefore = content.slice(lastIndex, match.index).trim();
      if (textBefore) {
        segments.push({ kind: 'text', text: textBefore });
      }
    }

    const widgetType = match[1];
    const rawData = match[2];

    // data属性のパース（JSON文字列 or Base64）
    const data = parseWidgetData(rawData);
    segments.push({ kind: 'widget', type: widgetType, data });

    lastIndex = match.index + match[0].length;
  }

  // タグが見つかった場合、残りのテキストを追加
  if (segments.length > 0) {
    if (lastIndex < content.length) {
      const remaining = content.slice(lastIndex).trim();
      if (remaining) {
        segments.push({ kind: 'text', text: remaining });
      }
    }
    return segments;
  }

  // 3. タグもJSONも検出されない → プレーンテキスト
  return [{ kind: 'text', text: content }];
}

/**
 * ウィジェットのdata属性値をパースする。
 * JSON文字列またはBase64エンコードされたJSONを受け付ける。
 */
function parseWidgetData(raw: string): unknown {
  // HTMLエンティティのデコード
  const decoded = raw
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&#39;/g, "'");

  // まずJSONとしてパース
  try {
    return JSON.parse(decoded);
  } catch {
    // Base64デコードを試行
    try {
      const jsonStr = atob(decoded);
      return JSON.parse(jsonStr);
    } catch {
      // パース不能 → 文字列としてそのまま返す
      return decoded;
    }
  }
}

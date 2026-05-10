import type { ComponentType } from 'react';
import { JsonViewer, JsonTreeViewer } from './JsonViewer';
import { CodeBlock } from './CodeBlock';
import { DataTable } from './DataTable';

/** ウィジェットコンポーネントが受け取る共通Props */
export interface WidgetProps<T = unknown> {
  data: T;
}

/** ウィジェットレジストリ: type名 → Reactコンポーネント のマッピング */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const widgetRegistry: Record<string, ComponentType<{ data: any }>> = {
  json: JsonViewer,
  json_tree: JsonTreeViewer,
  code: CodeBlock,
  data_table: DataTable,
  table: DataTable,
};

/**
 * type名に対応するウィジェットコンポーネントを取得する。
 * 未登録の場合は undefined を返す。
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function getWidget(type: string): ComponentType<{ data: any }> | undefined {
  return widgetRegistry[type];
}

/**
 * 新しいウィジェットをレジストリに登録する。
 * プラグインやカスタムツールから動的に追加する場合に使用。
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function registerWidget(type: string, component: ComponentType<{ data: any }>): void {
  widgetRegistry[type] = component;
}

export { JsonViewer, JsonTreeViewer, CodeBlock, DataTable };

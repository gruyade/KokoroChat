interface DataTableProps {
  data: {
    columns?: string[];
    rows: Record<string, unknown>[] | unknown[][];
    title?: string;
  };
}

/** テーブル形式でデータを表示するウィジェット */
export function DataTable({ data }: DataTableProps) {
  const { rows, title } = data;

  if (!rows || rows.length === 0) {
    return (
      <div className="rounded-md border border-border bg-muted/30 p-2.5 text-xs text-muted-foreground">
        データなし
      </div>
    );
  }

  // カラム名の決定: 明示的に指定されていればそれを使用、なければデータから推測
  const columns: string[] = data.columns
    ?? (Array.isArray(rows[0]) ? [] : Object.keys(rows[0] as Record<string, unknown>));

  const isArrayRows = Array.isArray(rows[0]);

  return (
    <div className="rounded-md border border-border bg-muted/30 overflow-hidden">
      {title && (
        <div className="px-2.5 py-1.5 border-b border-border/50 bg-muted/50">
          <span className="text-[11px] font-medium text-muted-foreground">{title}</span>
        </div>
      )}
      <div className="overflow-x-auto max-h-[300px] overflow-y-auto">
        <table className="w-full text-[11px] font-mono">
          {columns.length > 0 && (
            <thead>
              <tr className="border-b border-border/50 bg-muted/40">
                {columns.map((col, i) => (
                  <th
                    key={i}
                    className="px-2.5 py-1.5 text-left font-medium text-muted-foreground whitespace-nowrap"
                  >
                    {col}
                  </th>
                ))}
              </tr>
            </thead>
          )}
          <tbody>
            {rows.map((row, rowIdx) => (
              <tr
                key={rowIdx}
                className="border-b border-border/30 last:border-b-0 hover:bg-muted/20 transition-colors"
              >
                {isArrayRows
                  ? (row as unknown[]).map((cell, cellIdx) => (
                      <td key={cellIdx} className="px-2.5 py-1 text-muted-foreground whitespace-nowrap">
                        {formatCell(cell)}
                      </td>
                    ))
                  : columns.map((col, colIdx) => (
                      <td key={colIdx} className="px-2.5 py-1 text-muted-foreground whitespace-nowrap">
                        {formatCell((row as Record<string, unknown>)[col])}
                      </td>
                    ))
                }
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="px-2.5 py-1 border-t border-border/50 bg-muted/40">
        <span className="text-[10px] text-muted-foreground">{rows.length} 行</span>
      </div>
    </div>
  );
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return '—';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

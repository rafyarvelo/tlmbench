// Minimal stand-in for @tanstack/table-core, only for headless tests (the real one is blocked here).
export const getCoreRowModel = () => 'core';
export function createTable(o) {
  let opts = o;
  const initialState = { sorting: [], columnFilters: [], columnSizing: {} };
  const cols = () => opts.columns.map(def => {
    const id = def.id, st = () => opts.state || initialState;
    const col = {
      id, columnDef: def,
      getSize: () => (st().columnSizing || {})[id] || def.size || 120,
      getIsSorted: () => { const s = (st().sorting || []).find(x => x.id === id); return s ? (s.desc ? 'desc' : 'asc') : false; },
      getCanSort: () => def.enableSorting !== false,
      getSortIndex: () => (st().sorting || []).findIndex(x => x.id === id),
      getToggleSortingHandler: () => e => { const cur = col.getIsSorted(); const next = cur === 'asc' ? [{ id, desc: true }] : cur === 'desc' ? [] : [{ id, desc: false }]; opts.onStateChange(s => Object.assign({}, s, { sorting: next })); },
      getCanResize: () => false, getIsResizing: () => false,
      toggleSorting: (desc, multi) => opts.onStateChange(s => Object.assign({}, s, { sorting: [{ id, desc: !!desc }] })),
      clearSorting: () => opts.onStateChange(s => Object.assign({}, s, { sorting: (s.sorting || []).filter(x => x.id !== id) })),
      getCanFilter: () => def.enableColumnFilter !== false,
      getFilterValue: () => ((st().columnFilters || []).find(f => f.id === id) || {}).value,
      setFilterValue: v => opts.onStateChange(s => Object.assign({}, s, { columnFilters: (s.columnFilters || []).filter(f => f.id !== id).concat(v ? [{ id, value: v }] : []) })),
    };
    return col;
  });
  const t = {
    initialState,
    setOptions: fn => { opts = fn(opts); },
    getVisibleLeafColumns: () => cols(),
    getColumn: id => cols().find(c => c.id === id),
    getHeaderGroups: () => [{ headers: cols().map(c => ({ column: c, getSize: c.getSize, getResizeHandler: () => () => {} })) }],
    getTotalSize: () => cols().reduce((a, c) => a + c.getSize(), 0),
    getRowModel: () => ({ rows: (opts.data || []).map(r => ({ original: r, getVisibleCells: () => cols().map(c => ({ column: c, getValue: () => c.columnDef.accessorFn(r) })) })) }),
    setColumnSizing: sz => opts.onStateChange(s => Object.assign({}, s, { columnSizing: sz })),
    resetSorting: () => opts.onStateChange(s => Object.assign({}, s, { sorting: [] })),
    resetColumnFilters: () => opts.onStateChange(s => Object.assign({}, s, { columnFilters: [] })),
  };
  return t;
}

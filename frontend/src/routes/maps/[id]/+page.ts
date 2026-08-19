// Client-only: the per-user panel arrangement arrives from a separate request, so rendering
// on the server would paint the built-in layout and then shove every tile into place. There
// is nothing here worth indexing to trade for that.
export const ssr = false;
// The map page is client-only.
//
// Everything on it is interactive state the server cannot know: the pan and zoom, the
// selection, and above all the per-user panel arrangement, which arrives from a separate
// request. Rendering it on the server would paint the built-in layout first and then shove
// every tile into place once the real one loaded, which is a full-page layout shift on
// every visit. There is nothing here worth indexing, so there is nothing to trade for it.
export const ssr = false;

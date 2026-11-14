.pragma library

// Search-match helpers for the list cards.

function esc(t) {
    return t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// `text` as styled text with the first hit of `query` in bold.
function marked(text, query) {
    const q = query.trim();
    const i = q === "" ? -1 : text.toLowerCase().indexOf(q.toLowerCase());
    if (i < 0) return esc(text);
    return esc(text.slice(0, i)) + "<b>" + esc(text.slice(i, i + q.length)) + "</b>" + esc(text.slice(i + q.length));
}

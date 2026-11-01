.pragma library

// Reads `ai-usagebar usage --json`: { schema_version, entries: [{ id,
// display_name, plan, account, status, error, stale, sections: [...] }] }.
// A section of type "metric" is one quota window (label, percent, value,
// severity low|mid|high|critical, reset_at, window_secs); "text" sections are
// label/value notes such as credit balances. Returns null when the output is
// not a report, so the last good one stays on screen.
function parse(raw) {
    let d;
    try {
        d = JSON.parse(raw);
    } catch (e) {
        return null;
    }
    if (!d || !Array.isArray(d.entries))
        return null;
    return d.entries.filter(e => e && typeof e === "object").map(provider)
        .filter(p => p.metrics.length > 0 || p.notes.length > 0 || p.error !== "");
}

function provider(e) {
    const sections = Array.isArray(e.sections) ? e.sections.filter(s => s && typeof s === "object") : [];
    return {
        id: String(e.id || ""),
        name: String(e.display_name || e.id || ""),
        plan: String(e.plan || ""),
        account: String(e.account || ""),
        error: e.status === "error" ? String(e.error || "unavailable") : "",
        stale: e.stale === true,
        metrics: sections.filter(s => s.type === "metric").map(metric),
        notes: sections.filter(s => s.type === "text" && s.label && s.value && String(s.label).toLowerCase() !== "source")
            .map(s => ({ label: String(s.label), value: String(s.value) }))
    };
}

function metric(s) {
    const pct = s.percent === null || s.percent === undefined ? NaN : Number(s.percent);
    return {
        label: String(s.label || ""),
        // -1 when the window reports an amount (a balance) instead of a percent.
        pct: isFinite(pct) ? pct : -1,
        value: String(s.value || ""),
        severity: String(s.severity || "low"),
        reset: s.reset_at ? (Date.parse(s.reset_at) / 1000 || 0) : 0,
        window: Number(s.window_secs) || 0
    };
}

const ranks = { low: 0, mid: 1, high: 2, critical: 3 };

// The worst severity across every provider's windows, for the rail icon.
function worst(providers) {
    let r = 0;
    for (const p of providers)
        for (const m of p.metrics)
            r = Math.max(r, ranks[m.severity] || 0);
    return Object.keys(ranks)[r];
}

// "just now" / "45s ago" / "3m ago" / "2h ago" since `t`; "" when unknown.
function ago(t, nowSecs) {
    if (!(t > 0))
        return "";
    const s = Math.max(0, Math.floor(nowSecs - t));
    if (s < 5)
        return "just now";
    if (s < 60)
        return s + "s ago";
    if (s < 3600)
        return Math.floor(s / 60) + "m ago";
    return Math.floor(s / 3600) + "h ago";
}

// Usage against even spending: `expected` is the elapsed share of a
// `windowSecs` window ending at `resetTs` (percent), `points` how far `pct` is
// over it (negative is under). `null` when the window is unknown.
function pace(pct, resetTs, windowSecs, nowSecs) {
    if (!(resetTs > 0) || !(windowSecs > 0) || pct < 0)
        return null;
    const left = Math.max(0, Math.min(windowSecs, resetTs - nowSecs));
    const expected = (1 - left / windowSecs) * 100;
    return { expected: expected, points: Math.round(pct - expected) };
}

// "2h 14m" / "3d 4h" / "12m" until `resetTs`; "" once past or unknown.
function countdown(resetTs, nowSecs) {
    const s = Math.floor(resetTs - nowSecs);
    if (!(s > 0))
        return "";
    const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), m = Math.floor(s % 3600 / 60);
    if (d > 0)
        return d + "d " + h + "h";
    if (h > 0)
        return h + "h " + m + "m";
    return Math.max(1, m) + "m";
}

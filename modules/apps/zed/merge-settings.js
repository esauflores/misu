// Merges Misu's Zed defaults (argv[2], JSON) into the user's settings
// (argv[3], JSONC: Zed allows comments and trailing commas). Misu's keys win;
// the rest of the user's settings stay. Comments in the user's file are lost.
const fs = require("fs");
const [defaults, target] = process.argv.slice(2);

// Drops comments and trailing commas, leaving strings alone.
function jsonc(text) {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      let j = i + 1;
      while (j < text.length && text[j] !== '"') j += text[j] === "\\" ? 2 : 1;
      out += text.slice(i, j + 1);
      i = j;
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      out += "\n";
    } else if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 1;
    } else if (c === "}" || c === "]") {
      out = out.replace(/,\s*$/, "") + c; // drops a trailing comma
    } else {
      out += c;
    }
  }
  return JSON.parse(out);
}

const isObj = (v) => v && typeof v === "object" && !Array.isArray(v);
function merge(base, over) {
  for (const [k, v] of Object.entries(over)) base[k] = isObj(v) && isObj(base[k]) ? merge(base[k], v) : v;
  return base;
}

let user = {};
if (fs.existsSync(target)) {
  try { user = jsonc(fs.readFileSync(target, "utf8")); }
  catch (e) { console.error(`Skipping Zed defaults: ${target} is not valid JSON (${e.message})`); process.exit(0); }
}
fs.mkdirSync(require("path").dirname(target), { recursive: true });
fs.writeFileSync(target, JSON.stringify(merge(user, JSON.parse(fs.readFileSync(defaults, "utf8"))), null, 2) + "\n");

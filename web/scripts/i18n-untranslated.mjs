// Untranslated-surface audit for the RU panel.
//
// The `t()` catalog is guarded by check-i18n.mjs (en↔ru coverage). This tool
// covers the OTHER surface the user keeps catching by eye: config FIELD titles
// and descriptions, which don't live in the en catalog at all — when a field
// has no `config.field.<path>.label` / `.desc` catalog entry, the panel falls
// back to a computed humanized-English title / the raw Rust doc. So a plain
// en↔ru diff can never see them; you need the live schema's full field set.
//
// It walks the LIVE config JSON Schema (`OPTIONS /api/config`, no auth) to get
// every field path, collapses dynamic map keys to `*`, and reports which paths
// have no RU label / desc — grouped by top-level section, so the gap is a list
// you can work through instead of a screenshot lottery.
//
// Usage:
//   node scripts/i18n-untranslated.mjs                 # live pilot @ 127.0.0.1:42627
//   node scripts/i18n-untranslated.mjs <schemaUrl>     # another gateway
//   node scripts/i18n-untranslated.mjs --file <path>   # a saved OPTIONS /api/config dump
//   node scripts/i18n-untranslated.mjs --labels        # only missing labels (titles)
//   node scripts/i18n-untranslated.mjs --full          # list every missing path, not a per-section summary

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createJiti } from "jiti";
import { normalizeConfigFieldPath } from "./check-i18n.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const RU_PATH = resolve(__dirname, "../src/locales/ru.ts");
const DEFAULT_URL = "http://127.0.0.1:42627/api/config";

const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith("--")));
const positional = args.filter((a) => !a.startsWith("--"));
const fileIdx = args.indexOf("--file");
const schemaFile = fileIdx >= 0 ? args[fileIdx + 1] : null;
const schemaUrl = positional.find((p) => p !== schemaFile) ?? DEFAULT_URL;

async function loadSchema() {
  if (schemaFile) return JSON.parse(readFileSync(schemaFile, "utf8"));
  const res = await fetch(schemaUrl, { method: "OPTIONS" });
  if (!res.ok) throw new Error(`schema fetch ${schemaUrl} -> HTTP ${res.status}`);
  return res.json();
}

async function loadRu() {
  const jiti = createJiti(import.meta.url);
  const mod = await jiti.import(pathToFileURL(RU_PATH).href, { default: false });
  return mod.ru;
}

// RU field-description overrides, keyed by the exact EN Rust doc (see
// fieldDesc in i18n.ts). Empty when the file is absent.
function loadDescOverrides() {
  const p = resolve(__dirname, "../src/locales/fieldDescRu.json");
  try {
    return JSON.parse(readFileSync(p, "utf8"));
  } catch {
    return {};
  }
}

// Walk a schemars JSON Schema, collecting leaf field paths. Named-property
// objects recurse; map objects (additionalProperties) collapse their instance
// key to "*"; Option<T> (anyOf/oneOf with a null branch) recurses into the
// real branch; everything else (scalars, enums, arrays) is a leaf.
function collectPaths(root) {
  const defs = root.$defs ?? {};
  const out = new Map(); // path -> EN description (for the desc-hash fallback)
  const seen = new Set(); // "typeName@path" recursion guard

  const descOf = (s) =>
    s && typeof s === "object" && typeof s.description === "string" ? s.description : "";
  const branchesOf = (schema) => {
    const list = schema.anyOf ?? schema.oneOf ?? null;
    if (!list) return null;
    return list.filter((b) => b && b.type !== "null" && !(b.enum && b.enum.length === 1 && b.enum[0] === null));
  };

  function walk(schema, path, inheritedDesc) {
    if (!schema || typeof schema !== "object") {
      if (path) out.set(path, inheritedDesc);
      return;
    }
    const d = descOf(schema) || inheritedDesc;
    if (schema.$ref) {
      const name = schema.$ref.replace("#/$defs/", "");
      const key = `${name}@${path}`;
      if (seen.has(key)) return;
      seen.add(key);
      return walk(defs[name], path, d);
    }
    const branches = branchesOf(schema);
    if (branches) {
      // Recurse into any structured branch; if none, the field is a leaf.
      const structured = branches.filter(
        (b) => b.$ref || b.properties || b.additionalProperties,
      );
      if (structured.length === 0) {
        if (path) out.set(path, d);
        return;
      }
      for (const b of structured) walk(b, path, d);
      return;
    }
    if (schema.properties && Object.keys(schema.properties).length > 0) {
      for (const [k, v] of Object.entries(schema.properties)) {
        walk(v, path ? `${path}.${k}` : k, "");
      }
      return;
    }
    if (schema.additionalProperties && typeof schema.additionalProperties === "object") {
      walk(schema.additionalProperties, path ? `${path}.*` : "*", d);
      return;
    }
    // scalar / enum / array / empty object -> leaf
    if (path) out.set(path, d);
  }

  walk(root, "", "");
  return [...out.entries()];
}

// Humanized leaf label — mirrors humanizeFieldLabel in FieldForm.tsx (the
// title is a pure function of the leaf segment). Used to check the path-
// independent `config.fieldlabel.<humanized>` fallback catalog.
const LABEL_ACRONYMS = new Set(["api","url","uri","id","ip","ui","os","db","vm","ai","llm","mcp","tts","acp","ttl","sop","cpu","ram","gpu","dns","ssl","tls","json","toml","yaml","csv","sql","jwt","sse","ws","wss","http","https","rpc","grpc","cli","sdk","pdf","cwd","env"]);
function humanizeLeaf(path) {
  return (path.split(".").pop() ?? path)
    .replace(/[-_]/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => (LABEL_ACRONYMS.has(w.toLowerCase()) ? w.toUpperCase() : w.charAt(0).toUpperCase() + w.slice(1)))
    .join(" ");
}

function catalogHas(ru, descOverrides, path, kind, enDesc) {
  if (Object.prototype.hasOwnProperty.call(ru, `config.field.${path}.${kind}`)) return true;
  // A field TITLE also counts as covered when the humanized-label fallback
  // catalog (config.fieldlabel.<humanized>) has an entry — see fieldLabel().
  if (kind === "label") {
    return Object.prototype.hasOwnProperty.call(ru, `config.fieldlabel.${humanizeLeaf(path)}`);
  }
  // A DESCRIPTION counts as covered when the fieldDescRu overrides (keyed by
  // the exact EN text — see fieldDesc) contain it. A field with no EN
  // description at all has nothing to translate.
  if (kind === "desc") {
    if (!enDesc || !enDesc.trim()) return true;
    return Object.prototype.hasOwnProperty.call(descOverrides, enDesc);
  }
  return false;
}

function main() {
  return Promise.all([loadSchema(), loadRu()]).then(([schema, ru]) => {
    const descOverrides = loadDescOverrides();
    // Collapse map-instance keys the same way the catalog does
    // (normalizeConfigFieldPath: agents.<alias>, providers.models.<type>.<alias>,
    // authz.principals.<id>, ...), then dedup — so one catalog key covers all
    // runtime instances and we don't count a shared field once per instance.
    // Keep the EN description per normalized path for the desc-hash check.
    const descByPath = new Map();
    for (const [rawPath, enDesc] of collectPaths(schema)) {
      const n = normalizeConfigFieldPath(rawPath);
      if (!descByPath.has(n)) descByPath.set(n, enDesc);
    }
    const paths = [...descByPath.keys()].sort();
    const onlyLabels = flags.has("--labels");

    const missingLabel = paths.filter((p) => !catalogHas(ru, descOverrides, p, "label"));
    const missingDesc = paths.filter((p) => !catalogHas(ru, descOverrides, p, "desc", descByPath.get(p)));

    const bySection = (list) => {
      const m = new Map();
      for (const p of list) {
        const sec = p.split(".")[0];
        m.set(sec, (m.get(sec) ?? 0) + 1);
      }
      return [...m.entries()].sort((a, b) => b[1] - a[1]);
    };

    console.log(`Config field audit — ${paths.length} leaf field paths in the live schema`);
    console.log(`  RU labels present: ${paths.length - missingLabel.length}/${paths.length}`);
    console.log(`  RU descs  present: ${paths.length - missingDesc.length}/${paths.length}`);

    const report = (title, list) => {
      console.log(`\n${title}: ${list.length}`);
      if (flags.has("--full")) {
        for (const p of list) console.log(`  ${p}`);
      } else {
        for (const [sec, n] of bySection(list)) console.log(`  ${sec.padEnd(24)} ${n}`);
        console.log(`  (run with --full to list every path)`);
      }
    };

    report("Fields with NO RU title (config.field.*.label)", missingLabel);
    if (!onlyLabels) {
      report("Fields with NO RU description (config.field.*.desc)", missingDesc);
    }
  });
}

main().catch((err) => {
  console.error(String(err && err.message ? err.message : err));
  process.exit(1);
});

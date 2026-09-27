#!/usr/bin/env node
// One-shot codemod: rename web `--pc-*` design tokens and `pc-` Tailwind
// utility classes to Thunderbolt shadcn names. See _local/spec-web-thunderbolt-parity.md.
// Usage: node scripts/token-codemod.mjs [--dry] [path...]
import { readdirSync, statSync, readFileSync, writeFileSync } from 'node:fs'
import { join, extname } from 'node:path'

// --- CSS variable map (full token name -> target). Nuance: --pc-accent is the
// neutral ink -> --color-primary; --color-accent (beige) is a NEW token. ---
const VAR = {
  '--pc-bg-surface-subtle': '--color-surface-subtle',
  '--pc-bg-surface-elevated': '--color-surface-elevated',
  '--pc-bg-base': '--color-background',
  '--pc-bg-surface': '--color-card',
  '--pc-bg-elevated': '--color-secondary',
  '--pc-bg-input': '--color-input',
  '--pc-bg-code': '--color-code',
  '--pc-bg-sidebar': '--color-sidebar',
  '--pc-border-strong': '--color-border-strong',
  '--pc-border': '--color-border',
  '--pc-text-primary': '--color-foreground',
  '--pc-overlay': '--color-overlay',
  '--pc-bg-primary': '--color-background',
  '--pc-text-secondary': '--color-text-secondary',
  '--pc-text-muted': '--color-muted-foreground',
  '--pc-text-faint': '--color-text-faint',
  '--pc-text-link': '--color-text-link',
  '--pc-accent-foreground': '--color-primary-foreground',
  '--pc-accent-light': '--color-accent-light',
  '--pc-accent-dim': '--color-accent-dim',
  '--pc-accent-glow': '--color-accent-glow',
  '--pc-accent-rgb': '--color-accent-rgb',
  '--pc-accent-2': '--color-brand-2',
  '--pc-accent-gradient': '--gradient-brand',
  '--pc-accent': '--color-primary',
  '--pc-brand': '--color-brand',
  '--pc-shadow-sm': '--color-shadow-sm',
  '--pc-shadow-md': '--color-shadow-md',
  '--pc-focus': '--color-focus',
  '--pc-hover-strong': '--color-hover-strong',
  '--pc-hover': '--color-hover',
  '--pc-separator': '--color-separator',
  '--pc-scrollbar-thumb-hover': '--color-scrollbar-thumb-hover',
  '--pc-scrollbar-thumb': '--color-scrollbar-thumb',
  '--pc-scrollbar-track': '--color-scrollbar-track',
  '--pc-font-ui': '--font-sans',
  '--pc-font-mono': '--font-mono',
  '--pc-font-heading': '--font-heading',
  '--pc-font-chat-size': '--font-chat-size',
  '--pc-font-chat': '--font-chat',
  '--pc-font-size-mono': '--font-size-mono',
  '--pc-font-size': '--font-size',
  '--pc-text': '--color-foreground',
}

// --- Tailwind utility class suffix map (pc-<x> -> <tb>), for the @theme
// color tokens that produce bg-/text-/border-/ring-/etc utilities. ---
const CLASS = {
  'pc-bg-base': 'background',
  'pc-base': 'background',
  'pc-surface-subtle': 'surface-subtle',
  'pc-surface-elevated': 'surface-elevated',
  'pc-surface': 'card',
  'pc-elevated': 'secondary',
  'pc-input': 'input',
  'pc-code': 'code',
  'pc-border-strong': 'border-strong',
  'pc-border': 'border',
  'pc-text-secondary': 'text-secondary',
  'pc-text-muted': 'muted-foreground',
  'pc-text-faint': 'text-faint',
  'pc-text': 'foreground',
  'pc-accent-foreground': 'primary-foreground',
  'pc-accent-light': 'accent-light',
  'pc-accent-dim': 'accent-dim',
  'pc-accent-glow': 'accent-glow',
  'pc-accent': 'primary',
}
const PREFIXES = ['bg', 'text', 'border', 'ring-offset', 'ring', 'fill', 'stroke', 'from', 'via', 'to', 'outline', 'divide', 'caret', 'decoration', 'accent', 'shadow', 'placeholder']

const byLenDesc = (a, b) => b[0].length - a[0].length
const boundary = '(?![A-Za-z0-9-])'
const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

// var rules: full token, longest first, with a right boundary
const varRules = Object.entries(VAR).sort(byLenDesc).map(([from, to]) => [new RegExp(esc(from) + boundary, 'g'), to])
// class rules: <prefix>-<pc-suffix>, longest suffix first, with left `-`/quote and right boundary
const classRules = []
for (const [suf, tb] of Object.entries(CLASS).sort(byLenDesc)) {
  for (const p of PREFIXES) {
    classRules.push([new RegExp('\\b' + p + '-' + esc(suf) + boundary, 'g'), p + '-' + tb])
  }
}

function convert(text) {
  let out = text
  for (const [re, to] of varRules) out = out.replace(re, to)
  for (const [re, to] of classRules) out = out.replace(re, to)
  return out
}

const DRY = process.argv.includes('--dry')
const roots = process.argv.slice(2).filter((a) => !a.startsWith('--'))
const targets = roots.length ? roots : ['src']
const EXT = new Set(['.ts', '.tsx', '.css', '.js', '.jsx'])
const SKIP = new Set(['node_modules', 'dist', '.git'])

function walk(p, acc) {
  const st = statSync(p)
  if (st.isDirectory()) {
    if (SKIP.has(p.split(/[\\/]/).pop())) return
    for (const e of readdirSync(p)) walk(join(p, e), acc)
  } else if (EXT.has(extname(p))) acc.push(p)
}

const files = []
for (const t of targets) {
  const st = statSync(t)
  if (st.isFile()) files.push(t) // explicit file: include regardless of extension (e.g. themes.json)
  else walk(t, files)
}
let changedFiles = 0, changedVars = 0, changedClasses = 0
for (const f of files) {
  const src = readFileSync(f, 'utf8')
  let vHits = 0, cHits = 0
  let out = src
  for (const [re, to] of varRules) out = out.replace(re, (m) => (vHits++, to))
  for (const [re, to] of classRules) out = out.replace(re, (m) => (cHits++, to))
  if (out !== src) {
    changedFiles++; changedVars += vHits; changedClasses += cHits
    if (!DRY) writeFileSync(f, out)
  }
}
console.log(`${DRY ? '[dry] ' : ''}files changed: ${changedFiles}, var hits: ${changedVars}, class hits: ${changedClasses}`)

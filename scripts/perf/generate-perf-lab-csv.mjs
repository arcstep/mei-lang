#!/usr/bin/env node
/**
 * Deterministic synthetic CSV for ws-hello/apps/_perf-lab (Tier1.5 perf fixture).
 * Usage: node scripts/perf/generate-perf-lab-csv.mjs [--out ../workspaces/ws-hello/apps/_perf-lab/data]
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const defaultOut = resolve(__dirname, "../../workspaces/ws-hello/apps/_perf-lab/data");
const outDir = process.argv.includes("--out")
  ? resolve(process.argv[process.argv.indexOf("--out") + 1])
  : defaultOut;

function mulberry32(seed) {
  return function next() {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const rand = mulberry32(0x7e071ab);
const REGIONS = ["East", "North", "South", "West", "Central"];
const STATUSES = ["Open", "Closed", "Pending", "Escalated"];
const CATEGORIES = ["Inspection", "Permit", "Penalty", "Complaint", "Review"];

mkdirSync(outDir, { recursive: true });

const parkCount = 500;
const parks = [];
for (let i = 1; i <= parkCount; i += 1) {
  parks.push({ park_id: `P${String(i).padStart(4, "0")}`, park_name: `Park-${i}` });
}
writeFileSync(
  `${outDir}/parks.csv`,
  ["park_id,park_name", ...parks.map((p) => `${p.park_id},${p.park_name}`)].join("\n") + "\n",
);

const categories = CATEGORIES.map((name, idx) => ({
  category_id: `C${idx + 1}`,
  category_name: name,
}));
writeFileSync(
  `${outDir}/categories.csv`,
  ["category_id,category_name", ...categories.map((c) => `${c.category_id},${c.category_name}`)].join("\n") + "\n",
);

const entityCount = 5000;
const entities = [];
for (let i = 1; i <= entityCount; i += 1) {
  const park = parks[Math.floor(rand() * parks.length)];
  entities.push({
    entity_id: `E${String(i).padStart(5, "0")}`,
    entity_name: `Entity-${i}`,
    region: REGIONS[Math.floor(rand() * REGIONS.length)],
    park_id: park.park_id,
  });
}
writeFileSync(
  `${outDir}/entities.csv`,
  ["entity_id,entity_name,region,park_id", ...entities.map((e) => `${e.entity_id},${e.entity_name},${e.region},${e.park_id}`)].join("\n") + "\n",
);

const eventCount = 20000;
const events = [];
const startDate = new Date("2024-01-01T00:00:00Z");
for (let i = 1; i <= eventCount; i += 1) {
  const entity = entities[Math.floor(rand() * entities.length)];
  const dayOffset = Math.floor(rand() * 730);
  const d = new Date(startDate);
  d.setUTCDate(d.getUTCDate() + dayOffset);
  const event_date = d.toISOString().slice(0, 10);
  events.push({
    event_date,
    region: entity.region,
    status: STATUSES[Math.floor(rand() * STATUSES.length)],
    category: CATEGORIES[Math.floor(rand() * CATEGORIES.length)],
    amount: Math.floor(rand() * 5000) + 10,
    entity_id: entity.entity_id,
    park_id: entity.park_id,
  });
}
writeFileSync(
  `${outDir}/events.csv`,
  [
    "event_date,region,status,category,amount,entity_id,park_id",
    ...events.map(
      (e) =>
        `${e.event_date},${e.region},${e.status},${e.category},${e.amount},${e.entity_id},${e.park_id}`,
    ),
  ].join("\n") + "\n",
);

console.log(`perf-lab CSV written to ${outDir}`);
console.log(`  parks=${parks.length} entities=${entities.length} events=${events.length} categories=${categories.length}`);

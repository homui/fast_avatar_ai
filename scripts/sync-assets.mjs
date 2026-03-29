import { access, cp, mkdir, writeFile } from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const CUBISM_CORE_URL = "https://cubism.live2d.com/sdk-web/cubismcore/live2dcubismcore.min.js";

async function exists(target) {
  try {
    await access(target, fsConstants.F_OK);
    return true;
  } catch {
    return false;
  }
}

async function copyFirst(candidates, destination) {
  for (const candidate of candidates) {
    if (await exists(candidate)) {
      await mkdir(path.dirname(destination), { recursive: true });
      await cp(candidate, destination, { force: true });
      console.log(`copied ${path.relative(rootDir, candidate)} -> ${path.relative(rootDir, destination)}`);
      return;
    }
  }

  throw new Error(`No source file found for ${path.relative(rootDir, destination)}`);
}

async function ensureDownloaded(url, destination) {
  await mkdir(path.dirname(destination), { recursive: true });

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`);
  }

  const arrayBuffer = await response.arrayBuffer();
  await writeFile(destination, Buffer.from(arrayBuffer));
  console.log(`downloaded ${url} -> ${path.relative(rootDir, destination)}`);
}

const vendorFiles = [
  [
    [path.join(rootDir, "node_modules", "pixi.js", "dist", "pixi.min.js")],
    path.join(rootDir, "web", "vendor", "pixi.min.js"),
  ],
  [
    [
      path.join(rootDir, "node_modules", "@naari3", "pixi-live2d-display", "dist", "cubism5.min.js"),
    ],
    path.join(rootDir, "web", "vendor", "pixi-live2d-display.min.js"),
  ],
];

for (const [sources, destination] of vendorFiles) {
  await copyFirst(sources, destination);
}

await ensureDownloaded(
  CUBISM_CORE_URL,
  path.join(rootDir, "web", "vendor", "live2dcubismcore.min.js"),
);

const sourceLive2d = path.join(rootDir, "live2d");
const mirroredLive2d = path.join(rootDir, "web", "live2d");
await mkdir(path.dirname(mirroredLive2d), { recursive: true });
if (await exists(sourceLive2d)) {
  await cp(sourceLive2d, mirroredLive2d, { recursive: true, force: true });
  console.log(`mirrored ${path.relative(rootDir, sourceLive2d)} -> ${path.relative(rootDir, mirroredLive2d)}`);
}

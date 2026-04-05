import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { arch, platform, tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { build, context } from "esbuild";

const PROJECT_ROOT = dirname(fileURLToPath(import.meta.url));
const WATCH_MODE = process.argv.includes("--watch");
const PLATFORM = platform();
const ARCH = arch();

const TARGET_TRIPLES = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "win32-arm64": "aarch64-pc-windows-msvc",
  "win32-x64": "x86_64-pc-windows-msvc",
};

const triple = TARGET_TRIPLES[`${PLATFORM}-${ARCH}`];

if (!triple) {
  throw new Error(`Unsupported build target: ${PLATFORM}-${ARCH}`);
}

const isWindows = PLATFORM === "win32";
const outfile = resolve(
  PROJECT_ROOT,
  "..",
  "src-tauri",
  "binaries",
  `ai-service-${triple}${isWindows ? ".exe" : ""}`,
);
const tempDir = mkdtempSync(resolve(tmpdir(), "clashmind-ai-service-"));
const bundleFile = resolve(tempDir, "ai-service.cjs");
const blobFile = resolve(tempDir, "sea-prep.blob");
const seaConfigFile = resolve(tempDir, "sea-config.json");
const postjectCli = resolve(PROJECT_ROOT, "node_modules", "postject", "dist", "cli.js");

mkdirSync(dirname(outfile), { recursive: true });

function cleanup() {
  rmSync(tempDir, { force: true, recursive: true });
}

function packageExecutable() {
  writeFileSync(
    seaConfigFile,
    JSON.stringify(
      {
        main: bundleFile,
        output: blobFile,
        disableExperimentalSEAWarning: true,
        useCodeCache: false,
        useSnapshot: false,
      },
      null,
      2,
    ),
  );

  execFileSync(process.execPath, ["--experimental-sea-config", seaConfigFile], {
    stdio: "inherit",
  });

  copyFileSync(process.execPath, outfile);

  const postjectArgs = [
    postjectCli,
    outfile,
    "NODE_SEA_BLOB",
    blobFile,
    "--sentinel-fuse",
    "NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2",
  ];

  if (PLATFORM === "darwin") {
    postjectArgs.push("--macho-segment-name", "NODE_SEA");
  }

  execFileSync(process.execPath, postjectArgs, { stdio: "inherit" });

  if (!isWindows) {
    chmodSync(outfile, 0o755);
  }
}

const seaPackagingPlugin = {
  name: "sea-packaging",
  setup(buildContext) {
    buildContext.onEnd((result) => {
      if (result.errors.length > 0) {
        return;
      }

      packageExecutable();
      console.log(`Built ai-service executable: ${outfile}`);
    });
  },
};

const buildOptions = {
  entryPoints: [resolve(PROJECT_ROOT, "src", "index.ts")],
  outfile: bundleFile,
  bundle: true,
  format: "cjs",
  platform: "node",
  target: ["node22"],
  minify: !WATCH_MODE,
  tsconfig: resolve(PROJECT_ROOT, "tsconfig.json"),
  logLevel: "info",
  plugins: [seaPackagingPlugin],
};

process.on("exit", cleanup);
process.on("SIGINT", () => {
  cleanup();
  process.exit(130);
});
process.on("SIGTERM", () => {
  cleanup();
  process.exit(143);
});

if (WATCH_MODE) {
  const buildContext = await context(buildOptions);
  await buildContext.watch();
  console.log(`Watching ai-service sources -> ${outfile}`);
} else {
  await build(buildOptions);
}

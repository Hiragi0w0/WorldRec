// AI sidecar (worldrec-ai) を Tauri の externalBin 命名規則
// (worldrec-ai-<target triple>[.exe]) でビルドするスクリプト。
// 使い方: npm run build:sidecar
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

function hostTriple() {
  const output = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const match = output.match(/^host: (\S+)$/m);
  if (!match) {
    throw new Error("`rustc -vV` から host triple を取得できませんでした。");
  }
  return match[1];
}

const triple = process.env.WORLDREC_SIDECAR_TRIPLE ?? hostTriple();
const extension = triple.includes("windows") ? ".exe" : "";
const outDir = join(repoRoot, "src-tauri", "binaries");
mkdirSync(outDir, { recursive: true });
const outPath = join(outDir, `worldrec-ai-${triple}${extension}`);

execFileSync(
  "go",
  ["build", "-trimpath", "-ldflags", "-s -w", "-o", outPath, "./cmd/worldrec-ai"],
  { cwd: join(repoRoot, "sidecar"), stdio: "inherit" },
);

console.log(`AI sidecar をビルドしました: ${outPath}`);

import { spawn, spawnSync } from "node:child_process";
import {
  appendFileSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";

const root = process.cwd();
const publicCalico = join(root, "public", "calico");
const fixture = join(root, "tests", "fixtures", "calico-runtime-surface-probe.html");
const targetDir = process.env.CARGO_TARGET_DIR
  ? resolve(root, process.env.CARGO_TARGET_DIR)
  : join(root, "src-tauri", "target");
const artifactsDir = join(targetDir, "calico-probe-artifacts");
const resultPath = join(artifactsDir, "surface-probe.json");
const logPath = join(artifactsDir, "surface-probe.log");

const probePngs = {
  // A: opaque red at (0,0), transparent at (1,1), plus blue and half-alpha green.
  a: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAF0lEQVQI1wXBAQEAAACCIKb33EAkQ8EBOtwFfL5ifzgAAAAASUVORK5CYII=",
  // B: one opaque cyan pixel and three transparent pixels.
  b: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAD0lEQVQI12NgQAH///8HAAYMAv42RrTdAAAAAElFTkSuQmCC",
  // C: two opaque yellow pixels and two transparent pixels.
  c: "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVQI12P4/5/hPwMM/P/P8B8ANtsF+2wRsbEAAAAASUVORK5CYII=",
};

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    env: process.env,
    maxBuffer: 50 * 1024 * 1024,
    ...options,
  });
  if (existsSync(artifactsDir)) {
    appendFileSync(
      logPath,
      `$ ${command} ${args.join(" ")}\n${result.stdout || ""}${result.stderr || ""}`
    );
  }
  if (result.status !== 0) {
    const spawnError = result.error ? `\nspawn error: ${result.error.message}` : "";
    throw new Error(
      `${command} ${args.join(" ")} failed with status ${result.status}:${spawnError}`
    );
  }
  return result.stdout;
}

function assertCleanBeforeProbe() {
  const initialPublicStatus = run("git", ["status", "--short", "--", "public/calico"]).trim();
  const distDiff = spawnSync("git", ["diff", "--quiet", "--", "dist"], { cwd: root });
  if (distDiff.status !== 0) throw new Error("Tracked dist files must be clean before the probe.");
  for (const suffix of ["surface-probe.html", "probe-a.png", "probe-b.png", "probe-c.png"]) {
    if (existsSync(join(publicCalico, `runtime-${suffix}`))) {
      throw new Error(`Temporary probe input already exists: runtime-${suffix}`);
    }
  }
  return initialPublicStatus;
}

async function launchProbe(executable) {
  const chunks = [];
  const child = spawn(executable, [], {
    cwd: root,
    env: { ...process.env, PROMPT_PICKER_CALICO_PROBE_OUTPUT: resultPath },
    stdio: ["ignore", "ignore", "pipe"],
  });
  child.stderr.on("data", (chunk) => chunks.push(chunk));

  const exit = await new Promise((resolveExit, reject) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("probe process timeout after 45 seconds"));
    }, 45_000);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(new Error(`probe process launch failed: ${error.message}`));
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      resolveExit({ code, signal });
    });
  });
  const stderr = Buffer.concat(chunks).toString("utf8");
  appendFileSync(logPath, `${stderr}\nexit=${JSON.stringify(exit)}\n`);
  return { ...exit, stderr };
}

function validate(report) {
  if (report.error) throw new Error(`probe reported error: ${report.error}`);
  if (!report.compatibilityBackendDrawn) throw new Error("compatibility backend did not draw");
  if (!report.transparentPixelPreserved) throw new Error("transparent alpha was not preserved");
  if (!report.opaquePixelPreserved) throw new Error("opaque pixel was not preserved");
  if (!report.objectUrlRevoked) throw new Error("compatibility object URL was not revoked");
  if (report.createImageBitmapAvailable
      && (!report.preferredBackendDrawn || !report.imageBitmapCloseAvailable)) {
    throw new Error("preferred backend was available without successful draw and close support");
  }
  if (!report.rendererDiagnostics) throw new Error("production renderer diagnostics are missing");
  const maxima = report.rendererDiagnostics.maxima;
  for (const key of ["decodedSheetCount", "liveSurfaceCount"]) {
    if (maxima[key] > 2) throw new Error(`${key} exceeded 2: ${maxima[key]}`);
  }
  for (const key of ["pendingDecodeCount", "queuedRequestCount", "activeTimerCount"]) {
    if (maxima[key] > 1) throw new Error(`${key} exceeded 1: ${maxima[key]}`);
  }
  if (report.rendererDiagnostics.beforeDispose.staleGenerationDrawCount !== 0) {
    throw new Error("a stale renderer generation drew a frame");
  }
  const afterDispose = report.rendererDiagnostics.afterDispose;
  for (const key of [
    "decodedSheetCount", "liveSurfaceCount", "pendingDecodeCount",
    "queuedRequestCount", "activeTimerCount",
  ]) {
    if (afterDispose[key] !== 0) throw new Error(`${key} was not released on dispose`);
  }
  if (afterDispose.state !== "disposed" || afterDispose.visualReady !== false) {
    throw new Error("renderer did not enter a clean disposed state");
  }
}

const initialPublicStatus = assertCleanBeforeProbe();
rmSync(artifactsDir, { recursive: true, force: true });
mkdirSync(artifactsDir, { recursive: true });
writeFileSync(logPath, "");

try {
  copyFileSync(fixture, join(publicCalico, "runtime-surface-probe.html"));
  for (const [name, base64] of Object.entries(probePngs)) {
    writeFileSync(join(publicCalico, `runtime-probe-${name}.png`), Buffer.from(base64, "base64"));
  }

  run("npm", ["run", "tauri", "--", "build", "--debug", "--no-bundle"], {
    shell: process.platform === "win32",
  });
  const executable = join(targetDir, "debug", process.platform === "win32"
    ? "prompt-picker.exe"
    : "prompt-picker");
  if (!existsSync(executable)) throw new Error(`debug executable missing: ${executable}`);

  const processResult = await launchProbe(executable);
  console.log(`Probe JSON: ${resultPath}`);
  console.log(`Native stderr: ${processResult.stderr || "(empty)"}`);
  if (!existsSync(resultPath)) {
    throw new Error(`probe exited without JSON: ${JSON.stringify(processResult)}`);
  }
  let report;
  try {
    report = JSON.parse(readFileSync(resultPath, "utf8"));
  } catch (error) {
    throw new Error(`probe JSON parser failure: ${error.message}`);
  }
  validate(report);
  const maximaLine = `Renderer maxima: ${JSON.stringify(report.rendererDiagnostics.maxima)}`;
  appendFileSync(logPath, `${maximaLine}\n`);
  console.log(maximaLine);
  console.log(`Backend decision: ${report.createImageBitmapAvailable ? "preferred+compatibility" : "compatibility"}`);
} catch (error) {
  console.error(`Calico WebView probe failed: ${error.message}`);
  process.exitCode = 1;
} finally {
  for (const name of [
    "runtime-surface-probe.html",
    "runtime-probe-a.png",
    "runtime-probe-b.png",
    "runtime-probe-c.png",
  ]) {
    rmSync(join(publicCalico, name), { force: true });
    rmSync(join(root, "dist", "calico", name), { force: true });
  }
  spawnSync("git", ["restore", "--worktree", "--", "dist"], { cwd: root });
  const publicStatus = run("git", ["status", "--short", "--", "public/calico"]).trim();
  if (publicStatus !== initialPublicStatus) {
    throw new Error(`probe cleanup changed public/calico status:\n${publicStatus}`);
  }
  const distDiff = spawnSync("git", ["diff", "--quiet", "--", "dist"], { cwd: root });
  if (distDiff.status !== 0) throw new Error("probe cleanup left tracked dist files dirty");
}

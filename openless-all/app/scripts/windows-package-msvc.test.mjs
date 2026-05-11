import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const appRoot = join(scriptsDir, "..");
const scriptPath = join(scriptsDir, "windows-package-msvc.ps1");
const launcherPath = join(scriptsDir, "windows-package-msvc.cmd");
const imeBuildPath = join(scriptsDir, "windows-ime-build.ps1");
const imeRegisterPath = join(scriptsDir, "windows-ime-register.ps1");
const imeUnregisterPath = join(scriptsDir, "windows-ime-unregister.ps1");
const imeSolutionPath = join(appRoot, "windows-ime", "OpenLessIme.sln");
const imeProjectPath = join(appRoot, "windows-ime", "OpenLessIme.vcxproj");
const imeEditSessionPath = join(appRoot, "windows-ime", "src", "edit_session.cpp");
const imeTextServicePath = join(appRoot, "windows-ime", "src", "text_service.cpp");
const tauriConfigPath = join(appRoot, "src-tauri", "tauri.conf.json");
const wixFragmentPath = join(appRoot, "src-tauri", "wix", "openless-ime.wxs");

const script = readFileSync(scriptPath, "utf8");
const launcher = readFileSync(launcherPath, "utf8");
const imeBuild = readFileSync(imeBuildPath, "utf8");
const imeRegister = readFileSync(imeRegisterPath, "utf8");
const imeUnregister = readFileSync(imeUnregisterPath, "utf8");
const imeSolution = readFileSync(imeSolutionPath, "utf8");
const imeProject = readFileSync(imeProjectPath, "utf8");
const imeEditSession = readFileSync(imeEditSessionPath, "utf8");
const imeTextService = readFileSync(imeTextServicePath, "utf8");
const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, "utf8"));
const wixFragment = readFileSync(wixFragmentPath, "utf8");

const requiredFragments = [
  "Install-RustMsvcToolchain",
  "https://win.rustup.rs/x86_64",
  "stable-x86_64-pc-windows-msvc",
  "Find-VsDevCmd",
  "VsDevCmd.bat",
  "npm.cmd ci",
  "windows-ime-build.ps1",
  "OPENLESS_IME_DLL_X64",
  "OPENLESS_IME_DLL_X86",
  "OpenLessIme.dll",
  "tauri build -- --target x86_64-pc-windows-msvc --bundles msi",
  "Repair-TauriMsiBundle",
  "light.exe",
  "main.wixobj",
  "openless-ime.wixobj",
  "locale.wxl",
  "WebView2Loader.dll",
  "Compress-Archive",
  "Get-FileHash -Algorithm SHA256",
];

for (const fragment of requiredFragments) {
  assert.match(script, new RegExp(fragment.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")), `missing ${fragment}`);
}

assert.match(script, /\[switch\]\$SkipRustInstall/, "script should support opting out of Rust installation");
assert.match(script, /\[switch\]\$SkipNpmCi/, "script should support reusing existing node_modules");
assert.match(script, /\[switch\]\$CleanArtifacts/, "script should support cleaning the output directory");

assert.match(imeBuild, /\[string\]\$OutputDirectory/, "IME build should support a package-specific output directory");
assert.match(imeBuild, /\[string\]\$IntermediateDirectory/, "IME build should support a package-specific intermediate directory");
assert.match(imeBuild, /\[ValidateSet\("x64", "Win32"\)\]/, "IME build should support x64 and Win32 platforms");
assert.match(imeBuild, /\/p:Platform=\$Platform/, "IME build should pass Platform to MSBuild");
assert.match(imeBuild, /\$defaultOutputDirectory = Join-Path \$appRoot "windows-ime\\\$defaultPlatformFolder\\\$Configuration"/, "IME build should force stable default OutDir per platform");
assert.match(imeBuild, /\/p:OutDir=/, "IME build should pass OutDir to MSBuild");
assert.match(imeBuild, /\/p:IntDir=/, "IME build should pass IntDir to MSBuild");
assert.match(imeRegister, /windows-ime-build\.ps1/, "IME register should build before registering");
assert.doesNotMatch(imeRegister, /if \(-not \(Test-Path \$dll\)\)/, "IME register must rebuild stale DLLs, not only missing DLLs");
assert.match(imeRegister, /windows-ime-register/, "IME register should use a side-by-side staging output to avoid locked registered DLLs");
assert.match(imeRegister, /Get-Date/, "IME register should create a fresh staging output for each registration run");
assert.match(imeRegister, /\$PID/, "IME register should include the process id in the staging output to avoid path reuse");
assert.match(imeRegister, /-OutputDirectory/, "IME register should pass a staging output directory to the build script");
assert.match(imeRegister, /-IntermediateDirectory/, "IME register should pass a staging intermediate directory to the build script");
assert.match(imeRegister, /active-registration\.json/, "IME register should persist the staged DLL paths it registered");
assert.match(imeUnregister, /active-registration\.json/, "IME unregister should read the registered staged DLL manifest");
assert.match(imeUnregister, /windows-ime-register/, "IME unregister should target the same staging root used by register");
assert.match(imeUnregister, /ConvertFrom-Json/, "IME unregister should parse persisted registered DLL paths");
assert.doesNotMatch(imeUnregister, /windows-ime\\\$folder\\\$Configuration\\OpenLessIme\.dll/, "IME unregister must not only derive legacy build-output DLL paths");

assert.deepEqual(tauriConfig.bundle.windows.wix.fragmentPaths, ["wix/openless-ime.wxs"]);
assert.deepEqual(tauriConfig.bundle.windows.wix.componentRefs, [
  "OpenLessImeDllX64Component",
  "OpenLessImeDllX86Component",
]);

assert.match(imeSolution, /Release\|Win32/, "IME solution should include a Win32 Release configuration");
assert.match(imeProject, /Release\|Win32/, "IME project should include a Win32 Release configuration");
assert.match(imeTextService, /TF_E_SYNCHRONOUS/, "IME should detect hosts like Word that reject synchronous edit sessions");
assert.match(imeTextService, /TF_ES_ASYNC \| TF_ES_READWRITE/, "IME should retry Word-hosted commits with an async edit session");
assert.match(imeTextService, /WaitForSingleObject/, "IME pipe submit should wait for async edit-session completion");
assert.match(imeEditSession, /SetEvent/, "IME edit session should signal async completion back to the pipe submitter");
assert.match(imeEditSession, /Collapse\(edit_cookie, TF_ANCHOR_END\)/, "IME should collapse the committed range to its end after insertion");
assert.match(imeEditSession, /SetSelection\(edit_cookie, 1, &selection\)/, "IME should move the caret to the end of inserted text");
assert.match(imeEditSession, /TF_AE_END/, "IME should make the end of the committed text the active selection end");

assert.match(wixFragment, /DirectoryRef Id="INSTALLDIR"/, "WiX fragment should install into the app directory");
assert.match(wixFragment, /Component Id="OpenLessImeDllX64Component"/, "WiX fragment should define the x64 TSF DLL component");
assert.match(wixFragment, /Component Id="OpenLessImeDllX86Component"/, "WiX fragment should define the x86 TSF DLL component");
assert.match(wixFragment, /Source="src-tauri\\target\\windows-ime-msvc\\x64\\Release\\OpenLessIme\.dll"/, "WiX fragment should consume the package-built x64 IME DLL");
assert.match(wixFragment, /Source="src-tauri\\target\\windows-ime-msvc\\x86\\Release\\OpenLessIme\.dll"/, "WiX fragment should consume the package-built x86 IME DLL");
assert.match(wixFragment, /regsvr32\.exe/, "MSI should register and unregister the TSF DLL");
assert.match(wixFragment, /\[System64Folder\]regsvr32\.exe/, "MSI should register the x64 IME with 64-bit regsvr32");
assert.match(wixFragment, /\[WindowsFolder\]SysWOW64\\regsvr32\.exe/, "MSI should register the x86 IME with 32-bit regsvr32");
assert.match(wixFragment, /RegisterOpenLessImeX64/, "MSI should register x64 OpenLess IME during install");
assert.match(wixFragment, /RegisterOpenLessImeX86/, "MSI should register x86 OpenLess IME during install");
assert.match(wixFragment, /UnregisterOpenLessImeX64/, "MSI should unregister x64 OpenLess IME during uninstall");
assert.match(wixFragment, /UnregisterOpenLessImeX86/, "MSI should unregister x86 OpenLess IME during uninstall");

assert.match(launcher, /powershell\.exe/, "launcher should call powershell.exe");
assert.match(launcher, /-ExecutionPolicy Bypass/, "launcher should bypass execution policy for this process");
assert.match(launcher, /windows-package-msvc\.ps1/, "launcher should invoke the packaging script");
assert.match(launcher, /%SUPPLIED_ARGS%/, "launcher should forward user arguments");

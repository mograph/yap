# Builds Yap's Windows installers: an .exe (NSIS, the one to hand people) and an .msi
# (for deploying across a fleet). Both land in src-tauri\target\release\bundle.
#
# Unsigned installers work, but SmartScreen warns the first few hundred people who run one.
# To sign, get a code-signing certificate into your user store and set its thumbprint:
#   $env:YAP_WIN_CERT_THUMBPRINT = "aabbcc..."    # certmgr.msc -> Personal -> Certificates
#
# Then: powershell -ExecutionPolicy Bypass -File .\scripts\release-windows.ps1
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# rustup and CMake land on PATH only for new shells, so make sure this one can see them.
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\CMake\bin;$env:PATH"
foreach ($tool in 'cargo', 'cmake', 'pnpm') {
  if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
    throw "$tool is not on PATH. See the Windows section of README.md."
  }
}

# whisper.cpp runs on the GPU through Vulkan here, and its build needs the SDK's headers
# and vulkan-1.lib. The installer sets VULKAN_SDK machine-wide, but a shell opened before
# that won't have it yet, so fall back to the newest SDK on disk.
if (-not $env:VULKAN_SDK) {
  $machine = [Environment]::GetEnvironmentVariable('VULKAN_SDK', 'Machine')
  if ($machine) {
    $env:VULKAN_SDK = $machine
  } else {
    $newest = Get-ChildItem 'C:\VulkanSDK' -Directory -ErrorAction SilentlyContinue |
      Sort-Object Name -Descending | Select-Object -First 1
    if (-not $newest) { throw "Vulkan SDK not found. See the Windows section of README.md." }
    $env:VULKAN_SDK = $newest.FullName
  }
}
"Vulkan SDK: $($env:VULKAN_SDK)"
$env:PATH = "$env:VULKAN_SDK\Bin;$env:PATH"

# whisper-rs reads whisper.cpp's headers with bindgen, which needs libclang. Visual Studio
# ships one but doesn't advertise it, so point bindgen at it.
if (-not $env:LIBCLANG_PATH) {
  $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
  $roots = @()
  if (Test-Path $vswhere) { $roots += & $vswhere -products * -format value -property installationPath }
  $roots += 'C:\Program Files\LLVM'
  # Visual Studio ships an x86, an x64 and an ARM64 libclang. bindgen runs as a 64-bit
  # process, so the x64 one is the only one it can load.
  $dll = $roots | Where-Object { $_ } | ForEach-Object {
    Get-ChildItem -Path $_ -Recurse -Filter 'libclang.dll' -ErrorAction SilentlyContinue
  } | Sort-Object { if ($_.FullName -match '\\x64\\') { 0 } elseif ($_.FullName -match '\\ARM64\\') { 2 } else { 1 } } |
    Select-Object -First 1
  if (-not $dll) {
    throw "No libclang.dll found. Add the C++ Clang tools for Windows; see README.md."
  }
  $env:LIBCLANG_PATH = $dll.DirectoryName
  "libclang: $($env:LIBCLANG_PATH)"
}

$thumb = $env:YAP_WIN_CERT_THUMBPRINT
if ($thumb) {
  "Signing with certificate $thumb"
  # Tauri signs each bundle as it builds it, which is the only way the installer and the
  # .exe inside it both end up signed.
  $env:TAURI_BUNDLE_WINDOWS_CERTIFICATE_THUMBPRINT = $thumb
} else {
  "No YAP_WIN_CERT_THUMBPRINT set - building unsigned. SmartScreen will warn on first run."
}

pnpm tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

$bundle = 'src-tauri\target\release\bundle'
Get-ChildItem -Path $bundle -Recurse -Include '*.exe', '*.msi' -ErrorAction SilentlyContinue |
  ForEach-Object { "Ready: {0}  ({1:N1} MB)" -f $_.FullName, ($_.Length / 1MB) }

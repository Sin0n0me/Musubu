param(
    [string]$BuildDirectory = "target",
    [ValidateSet("debug", "release")][string]$Profile = "debug"
)
$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")
try {
    $cargoArguments = @("build", "-p", "musubu", "--locked", "--offline", "--target-dir", $BuildDirectory)
    if ($Profile -eq "release") { $cargoArguments += "--release" }
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
    $outputDirectory = Join-Path $BuildDirectory $Profile
    $executable = Join-Path $outputDirectory "ffi_smoke.exe"
    $object = Join-Path $outputDirectory "ffi_smoke.obj"
    & cl /nologo /std:c++17 /EHsc /W4 /WX /Icompiler/musubu/include tests/ffi_smoke.cpp "/Fo$object" "/Fe$executable" /link "/LIBPATH:$outputDirectory" musubu.dll.lib
    if ($LASTEXITCODE -ne 0) { throw "C++ build failed (run in an MSVC developer shell)" }
    & $executable
    if ($LASTEXITCODE -ne 0) { throw "FFI smoke checks failed" }
} finally {
    Pop-Location
}

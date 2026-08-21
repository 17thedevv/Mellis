$ErrorActionPreference = "Stop"

$Compiler = "..\..\build\Release\mellis.exe"
$Producer = "test_full_pipeline_mlib_producer.ms"

Write-Host "Compiling $Producer (Run 1)..."
& $Compiler $Producer -lib
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to compile run 1"
    exit 1
}

Rename-Item "test_full_pipeline_mlib_producer.mlib" "run1.mlib"

Write-Host "Compiling $Producer (Run 2)..."
& $Compiler $Producer -lib
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to compile run 2"
    exit 1
}

Rename-Item "test_full_pipeline_mlib_producer.mlib" "run2.mlib"

$hash1 = (Get-FileHash "run1.mlib").Hash
$hash2 = (Get-FileHash "run2.mlib").Hash

if ($hash1 -ne $hash2) {
    Write-Host "DETERMINISM TEST FAILED! Hashes do not match:"
    Write-Host "Run 1: $hash1"
    Write-Host "Run 2: $hash2"
    exit 1
}

Write-Host "DETERMINISM TEST PASSED! Hashes match ($hash1)"
exit 0

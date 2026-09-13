$ErrorActionPreference = "Stop"

$distDir = "dist\optioneclick"
$releaseDir = "releases"

if (Test-Path $distDir) {
    Remove-Item -Recurse -Force $distDir
}
if (-not (Test-Path $releaseDir)) {
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
}

New-Item -ItemType Directory -Force -Path $distDir | Out-Null

Copy-Item "target\release\optioneclick.exe" -Destination "$distDir\optioneclick.exe"
Copy-Item "README.md" -Destination "$distDir\README.md"
Copy-Item "LICENSE" -Destination "$distDir\LICENSE"
Copy-Item "THIRD_PARTY_LICENSES.md" -Destination "$distDir\THIRD_PARTY_LICENSES.md"

$zipFile = "$releaseDir\optioneclick-v0.1.0-windows-x64.zip"
if (Test-Path $zipFile) {
    Remove-Item -Force $zipFile
}

Compress-Archive -Path "$distDir\*" -DestinationPath $zipFile -Force

$zipHash = (Get-FileHash -Path $zipFile -Algorithm SHA256).Hash.ToLower()
"$zipHash  optioneclick-v0.1.0-windows-x64.zip" | Out-File -Encoding ascii -FilePath "$zipFile.sha256"

# Also copy the standalone exe directly to releases so users can download it without unzipping
Copy-Item "target\release\optioneclick.exe" -Destination "$releaseDir\optioneclick.exe"
$exeHash = (Get-FileHash -Path "$releaseDir\optioneclick.exe" -Algorithm SHA256).Hash.ToLower()
"$exeHash  optioneclick.exe" | Out-File -Encoding ascii -FilePath "$releaseDir\optioneclick.exe.sha256"

Write-Host "Packaging complete:"
Get-ChildItem $releaseDir | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize

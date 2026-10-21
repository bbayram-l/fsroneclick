$ErrorActionPreference = "Stop"

$distDir = "dist\fsroneclick"
$releaseDir = "releases"
$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$zipName = "fsroneclick-v$version-windows-x64.zip"

if (Test-Path $distDir) {
    Remove-Item -Recurse -Force $distDir
}
if (-not (Test-Path $releaseDir)) {
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
}

New-Item -ItemType Directory -Force -Path $distDir | Out-Null

Copy-Item "target\release\fsroneclick.exe" -Destination "$distDir\fsroneclick.exe"
Copy-Item "README.md" -Destination "$distDir\README.md"
Copy-Item "LICENSE" -Destination "$distDir\LICENSE"
Copy-Item "THIRD_PARTY_LICENSES.md" -Destination "$distDir\THIRD_PARTY_LICENSES.md"

$zipFile = "$releaseDir\$zipName"
if (Test-Path $zipFile) {
    Remove-Item -Force $zipFile
}

Compress-Archive -Path "$distDir\*" -DestinationPath $zipFile -Force

$zipHash = (Get-FileHash -Path $zipFile -Algorithm SHA256).Hash.ToLower()
"$zipHash  $zipName" | Out-File -Encoding ascii -FilePath "$zipFile.sha256"

# Also copy the standalone exe directly to releases so users can download it without unzipping
Copy-Item "target\release\fsroneclick.exe" -Destination "$releaseDir\fsroneclick.exe"
$exeHash = (Get-FileHash -Path "$releaseDir\fsroneclick.exe" -Algorithm SHA256).Hash.ToLower()
"$exeHash  fsroneclick.exe" | Out-File -Encoding ascii -FilePath "$releaseDir\fsroneclick.exe.sha256"

# Clean up any legacy optioneclick files in releases folder
Get-ChildItem $releaseDir -Filter "*optioneclick*" | Remove-Item -Force

Write-Host "Packaging complete:"
Get-ChildItem $releaseDir | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize

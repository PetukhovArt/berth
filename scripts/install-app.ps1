param(
    [string]$Repo,
    [string]$Root,
    [string]$Ref = 'main',
    [string]$ShortcutDir,
    [switch]$Force,
    [switch]$Launch,
    [switch]$Rollback,
    [switch]$UpdateDaemon,
    [switch]$CheckVoice
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)

if (-not $Root) { $Root = if ($env:BERTH_APP_ROOT) { $env:BERTH_APP_ROOT } else { Join-Path $env:LOCALAPPDATA 'Berth' } }
if (-not $ShortcutDir) { $ShortcutDir = if ($env:BERTH_START_MENU) { $env:BERTH_START_MENU } else { Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs' } }

$appDir = Join-Path $Root 'app'
$buildDir = Join-Path $Root 'build'
$current = Join-Path $appDir 'current'
$previous = Join-Path $appDir 'previous'
$statusPath = Join-Path $buildDir 'status.json'
$exeName = 'berth-desktop.exe'

function Read-Build([string]$dir) {
    $file = Join-Path $dir 'build.json'
    if (Test-Path -LiteralPath $file) { Get-Content -Raw -LiteralPath $file | ConvertFrom-Json }
}

function Get-LinkTarget([string]$link) {
    if (-not (Test-Path -LiteralPath $link)) { return $null }
    $target = [string](Get-Item -LiteralPath $link).Target
    if ($target) { $target } else { $null }
}

function Set-Link([string]$link, [string]$target) {
    if (Test-Path -LiteralPath $link) { [IO.Directory]::Delete($link) }
    New-Item -ItemType Junction -Path $link -Target $target | Out-Null
}

function Set-Shortcut {
    New-Item -ItemType Directory -Force -Path $ShortcutDir | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut((Join-Path $ShortcutDir 'Berth.lnk'))
    $link.TargetPath = Join-Path $current $exeName
    $link.WorkingDirectory = $current
    $link.IconLocation = "$(Join-Path $current $exeName),0"
    $link.Description = 'Berth'
    $link.Save()
}

function Start-Berth {
    Start-Process -FilePath (Join-Path $current $exeName) -WorkingDirectory $current
}

function Find-Libclang {
    $names = 'libclang.dll', 'clang.dll'
    if ($env:LIBCLANG_PATH) {
        if (Test-Path -LiteralPath $env:LIBCLANG_PATH -PathType Leaf) { return Split-Path -Parent $env:LIBCLANG_PATH }
        if ($names | Where-Object { Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH $_) }) { return $env:LIBCLANG_PATH }
        return $null
    }
    $dirs = @('C:\Program Files\LLVM\bin', 'C:\Program Files (x86)\LLVM\bin', 'C:\LLVM\bin', (Join-Path $HOME 'scoop\apps\llvm\current\bin'))
    $dirs += @(Get-ChildItem -Path 'C:\Program Files*\Microsoft Visual Studio\*\*\VC\Tools\Llvm' -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { (Join-Path $_.FullName 'bin'), (Join-Path $_.FullName 'x64\bin') })
    $dirs | Where-Object { Test-Path -LiteralPath (Join-Path $_ 'libclang.dll') } | Select-Object -First 1
}

function Find-CppVisualStudio {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { return $null }
    $cppTools = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
    $path = @(& $vswhere -latest -products * -requires $cppTools -property installationPath)[0]
    if (-not $path) { return $null }
    $version = @(& $vswhere -latest -products * -requires $cppTools -property installationVersion)[0]
    $newest = @(& $vswhere -latest -products * -property installationPath)[0]
    [pscustomobject]@{ Path = $path; Major = [int]("$version" -split '\.')[0]; Newest = $path -eq $newest }
}

function Get-VoiceSetup {
    $missing = @()
    $buildEnv = [ordered]@{}
    $libclang = Find-Libclang
    if (-not $libclang) {
        $missing += 'LLVM (winget install LLVM.LLVM)'
    } elseif (-not $env:CLANG_PATH -and -not (Get-Command clang -ErrorAction SilentlyContinue) -and (Test-Path -LiteralPath (Join-Path $libclang 'clang.exe'))) {
        $buildEnv.CLANG_PATH = Join-Path $libclang 'clang.exe'
    }
    $cmake = Get-Command cmake -ErrorAction SilentlyContinue
    if (-not $cmake) { $missing += 'CMake (winget install Kitware.CMake)' }
    $vs = Find-CppVisualStudio
    if (-not $vs) { $missing += 'the C++ workload of Visual Studio Build Tools' }
    if ($cmake -and $vs -and -not $env:CMAKE_GENERATOR) {
        $generator = @{ 16 = 'Visual Studio 16 2019'; 17 = 'Visual Studio 17 2022' }[$vs.Major]
        $first = @(& $cmake.Source --version 2>$null)[0]
        $oldCmake = ("$first" -match '(\d+)\.(\d+)') -and ([version]"$($Matches[1]).$($Matches[2])" -lt [version]'3.21')
        if ($oldCmake -and $vs.Major -ge 17) { $missing += 'CMake 3.21 or newer (winget install Kitware.CMake)' }
        elseif (($oldCmake -or -not $vs.Newest) -and $generator) { $buildEnv.CMAKE_GENERATOR = $generator }
    }
    if ($vs -and -not $vs.Newest -and -not $env:VCToolsInstallDir) {
        $msvc = Get-ChildItem -LiteralPath (Join-Path $vs.Path 'VC\Tools\MSVC') -Directory -ErrorAction SilentlyContinue |
            Sort-Object { [version]$_.Name } | Select-Object -Last 1
        if ($msvc) { $buildEnv.VCToolsInstallDir = "$($msvc.FullName)\" }
    }
    $fallback = if ($missing.Count) { "voice input: building without the local speech engine, so the app uses the browser's speech recognition; install $($missing -join ' and ') to get it" }
    [pscustomobject]@{ Fallback = $fallback; Env = $buildEnv }
}

function Format-BuildEnv($buildEnv) {
    ($buildEnv.Keys | ForEach-Object { "$_=$($buildEnv[$_])" }) -join ', '
}

if ($CheckVoice) {
    $setup = Get-VoiceSetup
    if ($setup.Fallback) { Write-Host $setup.Fallback }
    else { Write-Host "voice input: libclang, CMake and MSVC found, building with the local speech engine$(if ($setup.Env.Count) { " ($(Format-BuildEnv $setup.Env))" })" }
    exit 0
}

if ($Rollback) {
    $now = Get-LinkTarget $current
    $then = Get-LinkTarget $previous
    if (-not $then -or -not (Test-Path -LiteralPath (Join-Path $then $exeName))) { throw "no previous build under $appDir" }
    Set-Link $current $then
    if ($now) { Set-Link $previous $now }
    $build = Read-Build $current
    Write-Host "current build: $($build.buildId) $($build.subject)"
    if ($Launch) { Start-Berth }
    exit 0
}

if ($UpdateDaemon) {
    $built = Join-Path $current 'berth.exe'
    if (-not (Test-Path -LiteralPath $built)) { throw "no berth.exe in $current" }
    $prefix = if ($env:BERTH_BIN_DIR) { $env:BERTH_BIN_DIR } else { Join-Path $HOME '.cargo\bin' }
    New-Item -ItemType Directory -Force -Path $prefix | Out-Null
    $installed = Join-Path $prefix 'berth.exe'
    $old = Join-Path $prefix 'berth.old.exe'
    if (Test-Path -LiteralPath $old) { Remove-Item -LiteralPath $old -Force -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $installed) { Move-Item -LiteralPath $installed -Destination $old -Force }
    Copy-Item -LiteralPath $built -Destination $installed
    & $installed --version
    & $installed kill
    Start-Process -FilePath $installed -ArgumentList 'daemon' -WindowStyle Hidden
    $daemonDir = Join-Path $Root 'daemon'
    New-Item -ItemType Directory -Force -Path $daemonDir | Out-Null
    Copy-Item -LiteralPath (Join-Path $current 'build.json') -Destination (Join-Path $daemonDir 'build.json') -Force
    Write-Host "daemon restarted from $installed"
    exit 0
}

if (-not $Repo) {
    $checkout = Join-Path $PSScriptRoot '..'
    $own = Read-Build $PSScriptRoot
    if (Test-Path -LiteralPath (Join-Path $checkout '.git')) { $Repo = (Resolve-Path $checkout).Path }
    elseif ($own) { $Repo = $own.sourceRepo }
    else { throw 'pass -Repo <path to the berth checkout>' }
}
$Repo = (Resolve-Path -LiteralPath $Repo).Path

New-Item -ItemType Directory -Force -Path $buildDir, $appDir, (Join-Path $buildDir 'logs') | Out-Null
$startedAt = (Get-Date).ToString('o')
$status = [ordered]@{ state = 'running'; step = 'start'; pid = $PID; repo = $Repo; startedAt = $startedAt; steps = [ordered]@{ start = $startedAt } }

function Save-Status {
    $temp = "$statusPath.tmp"
    [IO.File]::WriteAllText($temp, ($status | ConvertTo-Json -Depth 4), $utf8)
    Move-Item -LiteralPath $temp -Destination $statusPath -Force
}

function Set-Step([string]$step) {
    $status.step = $step
    $status.steps[$step] = (Get-Date).ToString('o')
    Save-Status
    Add-Log "== $step"
}

function Add-Log([string]$line) {
    [IO.File]::AppendAllText($log, "$line`r`n", $utf8)
}

function Invoke-Logged([string]$exe, [string[]]$arguments) {
    Add-Log "> $exe $($arguments -join ' ')"
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $exe @arguments 2>&1 | ForEach-Object { Add-Log "$_" }
    $code = $LASTEXITCODE
    $ErrorActionPreference = $saved
    if ($code -ne 0) { throw "$exe $($arguments -join ' ') exited with code $code" }
}

$log = Join-Path $buildDir "logs\$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
$status.log = $log
Save-Status

try {
    $commit = (git -C $Repo rev-parse --verify "$Ref^{commit}").Trim()
    if ($LASTEXITCODE -ne 0 -or -not $commit) { throw "no commit $Ref in $Repo" }
    $subject = (git -C $Repo log -1 --format=%s $commit) -join ' '
    $status.commit = $commit
    $status.subject = $subject
    Save-Status

    $installedBuild = Read-Build $current
    if (-not $Force -and $installedBuild -and $installedBuild.commit -eq $commit) {
        $status.state = 'done'
        $status.upToDate = $true
        $status.buildId = $installedBuild.buildId
        $status.finishedAt = (Get-Date).ToString('o')
        Save-Status
        Write-Host "already up to date: $($installedBuild.buildId)"
        if ($Launch) { Start-Berth }
        exit 0
    }

    Set-Step 'export'
    $src = Join-Path $buildDir 'src'
    New-Item -ItemType Directory -Force -Path $src | Out-Null
    $gitDir = (git -C $Repo rev-parse --absolute-git-dir).Trim()
    $env:GIT_INDEX_FILE = Join-Path $buildDir 'src.index'
    try {
        Invoke-Logged 'git' @("--git-dir=$gitDir", "--work-tree=$src", 'read-tree', '--reset', '-u', $commit)
    } finally {
        Remove-Item Env:\GIT_INDEX_FILE
    }

    $env:CARGO_TARGET_DIR = Join-Path $buildDir 'target'
    $env:CARGO_PROFILE_RELEASE_LTO = 'false'
    $env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '16'
    $env:CARGO_PROFILE_RELEASE_INCREMENTAL = 'true'
    $env:CARGO_PROFILE_RELEASE_OPT_LEVEL = '2'

    $daemonExe = Join-Path $env:CARGO_TARGET_DIR 'release\berth.exe'
    $installedDaemon = Join-Path $current 'berth.exe'
    if ($installedBuild -and (Test-Path -LiteralPath $installedDaemon)) {
        $ErrorActionPreference = 'Continue'
        git -C $Repo diff --quiet $installedBuild.commit $commit -- crates vendor Cargo.toml Cargo.lock rust-toolchain.toml .cargo 2>$null
        $unchanged = $LASTEXITCODE -eq 0
        $ErrorActionPreference = 'Stop'
        if ($unchanged) { $daemonExe = $installedDaemon }
    }

    Push-Location $src
    try {
        Set-Step 'deps'
        Invoke-Logged 'pnpm' @('install', '--frozen-lockfile', '--config.confirmModulesPurge=false')
        Set-Step 'tokens'
        Invoke-Logged 'pnpm' @('build:tokens')
        Set-Step 'app'
        $voiceSetup = Get-VoiceSetup
        $voiceFallback = $voiceSetup.Fallback
        $appBuild = @('--filter', '@berth/desktop', 'exec', 'tauri', 'build', '--no-bundle')
        if ($voiceFallback) {
            Add-Log $voiceFallback
            Write-Host $voiceFallback
        } else {
            $appBuild += @('--features', 'voice')
            foreach ($name in $voiceSetup.Env.Keys) { Set-Item -Path "Env:$name" -Value $voiceSetup.Env[$name] }
            if ($voiceSetup.Env.Count) { Add-Log "voice build environment: $(Format-BuildEnv $voiceSetup.Env)" }
        }
        Invoke-Logged 'pnpm' $appBuild
        if ($daemonExe -eq $installedDaemon) {
            Add-Log "daemon sources unchanged since $($installedBuild.commit), reusing $installedDaemon"
        } else {
            Set-Step 'daemon'
            Invoke-Logged 'cargo' @('build', '--release', '--locked', '-p', 'berth')
        }
    } finally {
        Pop-Location
    }

    Set-Step 'install'
    $release = Join-Path $env:CARGO_TARGET_DIR 'release'
    $protocolLine = Select-String -LiteralPath (Join-Path $src 'crates\berth-core\src\protocol.rs') -Pattern 'pub const PROTOCOL_VERSION: u32 = (\d+);' | Select-Object -First 1
    $protocol = [int]$protocolLine.Matches[0].Groups[1].Value
    $buildId = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$($commit.Substring(0, 8))"
    $target = Join-Path $appDir $buildId
    $staging = "$target.partial"
    if (Test-Path -LiteralPath $staging) { Remove-Item -Recurse -Force -LiteralPath $staging }
    New-Item -ItemType Directory -Force -Path $staging | Out-Null
    Copy-Item -LiteralPath (Join-Path $release $exeName) -Destination $staging
    if (-not $voiceFallback) {
        foreach ($dll in 'sherpa-onnx-c-api.dll', 'onnxruntime.dll') { Copy-Item -LiteralPath (Join-Path $release $dll) -Destination $staging }
    }
    Copy-Item -LiteralPath $daemonExe -Destination $staging
    Copy-Item -LiteralPath (Join-Path $src 'scripts\install-app.ps1') -Destination $staging
    $build = [ordered]@{ buildId = $buildId; commit = $commit; subject = $subject; sourceRepo = $Repo; builtAt = (Get-Date).ToString('o'); protocol = $protocol }
    [IO.File]::WriteAllText((Join-Path $staging 'build.json'), ($build | ConvertTo-Json), $utf8)
    Rename-Item -LiteralPath $staging -NewName $buildId

    $was = Get-LinkTarget $current
    Set-Link $current $target
    if ($was) { Set-Link $previous $was }
    Set-Shortcut

    $keep = @($target, $was) | Where-Object { $_ } | ForEach-Object { [IO.Path]::GetFullPath($_).TrimEnd('\') }
    $running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path } | ForEach-Object { [IO.Path]::GetDirectoryName($_.Path) })
    Get-ChildItem -LiteralPath $appDir -Directory | Where-Object {
        $_.Name -notin @('current', 'previous') -and -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)
    } | ForEach-Object {
        $full = $_.FullName.TrimEnd('\')
        if ($keep -contains $full -or $running -contains $full) { return }
        Remove-Item -Recurse -Force -LiteralPath $full -ErrorAction SilentlyContinue
    }

    $status.state = 'done'
    $status.step = 'install'
    $status.buildId = $buildId
    $status.protocol = $protocol
    $status.finishedAt = (Get-Date).ToString('o')
    Save-Status
    Add-Log "installed $buildId into $target"
    Write-Host "installed $buildId ($subject) into $target"
    if ($Launch) { Start-Berth }
} catch {
    $status.state = 'failed'
    $status.error = "$_"
    $status.finishedAt = (Get-Date).ToString('o')
    Save-Status
    Add-Log "failed: $_"
    Write-Error "build failed: $_ (log: $log)" -ErrorAction Continue
    exit 1
}

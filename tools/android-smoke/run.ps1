<#
Runs the Kotlin smoke test (core/smoke/kotlin/Main.kt) on attached Android phones, on
every ABI each phone supports, through app_process: no app is installed, the screen is
never touched and no setting changes. Phase 6, owner-approved (7 November 2026).

    powershell -File tools/android-smoke/run.ps1 -Bundle <dir of the CI artifact encore-android-smoke>

For each phone and ABI it starts a fresh Jellyfin replay (tools/ci/replay_jellyfin.py)
and the throwaway TLS server (tls_server.py) on this PC, reaches them from the phone
with `adb reverse`, runs the test from /data/local/tmp/encore-smoke-<id>, and checks
"library flow OK", "https guard OK" and the parity value CI computed. Everything it
put on a phone is removed at the end, and the reverse rules with it.

It refuses to start while something else drives the phones (an adb shell, install or
logcat, Gradle or Flutter on this PC), unless -Force.
#>
param(
    [Parameter(Mandatory = $true)][string]$Bundle,
    [string]$Adb = "",
    [string[]]$Devices = @(),
    [string]$Out = "",
    [string]$WorkRoot = [IO.Path]::GetTempPath(),
    [switch]$Force
)

$ErrorActionPreference = "Stop"
# adb from -Adb, else the Android SDK (ANDROID_HOME), else PATH.
if (-not $Adb) {
    $sdkAdb = if ($env:ANDROID_HOME) { Join-Path $env:ANDROID_HOME "platform-tools\adb.exe" } else { "" }
    $Adb = if ($sdkAdb -and (Test-Path $sdkAdb)) { $sdkAdb } else { (Get-Command adb -ErrorAction Stop).Source }
}
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$fixture = Join-Path $repo "tools\fixtures\jellyfin\recorded.json"
$expected = (Get-Content (Join-Path $Bundle "expected-parity.txt") -Raw).Trim()
if ($expected -notmatch '^[0-9a-f]{8}$') { throw "expected-parity.txt holds '$expected'" }

function Invoke-Adb([string]$serial, [string[]]$a) {
    # adb writes progress to stderr; Windows PowerShell 5.1 would stop on it.
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $o = & $Adb -s $serial @a 2>&1 | ForEach-Object { "$_" }
    }
    finally {
        $ErrorActionPreference = $prev
    }
    return ($o -join "`n")
}

# 1. Is anyone else using the phones?
if (-not $Force) {
    $busy = @(Get-CimInstance Win32_Process | Where-Object {
            ($_.Name -eq "adb.exe" -and $_.CommandLine -match '\s(shell|install|logcat|exec-out|push|pull)\b') -or
            ($_.Name -match '^(java|gradle|flutter|dart)(\.exe)?$' -and $_.CommandLine -match '(?i)gradle|flutter|daemon')
        })
    if ($busy.Count -gt 0) {
        $busy | ForEach-Object { Write-Host "busy: $($_.Name) $($_.ProcessId) $($_.CommandLine)" }
        throw "Something else is using the phones; try later (or -Force)."
    }
}

$attached = @(& $Adb devices | Select-String -Pattern '^(\S+)\s+device$' | ForEach-Object { $_.Matches[0].Groups[1].Value })
if ($Devices.Count -gt 0) { $attached = @($attached | Where-Object { $Devices -contains $_ }) }
if ($attached.Count -eq 0) { throw "No phone attached." }

$work = Join-Path $WorkRoot ("android-smoke-run-" + [guid]::NewGuid().ToString("N").Substring(0, 8))

New-Item -ItemType Directory -Force $work | Out-Null
$results = @()

function Start-Helper([string[]]$pyArgs, [string]$urlFile) {
    $p = Start-Process -FilePath "python" -ArgumentList (@("-I", "-B") + $pyArgs + @($urlFile)) -PassThru -WindowStyle Hidden
    for ($i = 0; $i -lt 300; $i++) {
        if ((Test-Path $urlFile) -and (Get-Item $urlFile).Length -gt 0) { break }
        Start-Sleep -Milliseconds 100
    }
    if (-not ((Test-Path $urlFile) -and (Get-Item $urlFile).Length -gt 0)) { Stop-Process -Id $p.Id -Force; throw "helper did not start: $pyArgs" }
    return @{ proc = $p; url = (Get-Content $urlFile -Raw).Trim() }
}

foreach ($serial in $attached) {
    $model = (Invoke-Adb $serial @("shell", "getprop", "ro.product.model")).Trim()
    $release = (Invoke-Adb $serial @("shell", "getprop", "ro.build.version.release")).Trim()
    $sdk = (Invoke-Adb $serial @("shell", "getprop", "ro.build.version.sdk")).Trim()
    $abis = (Invoke-Adb $serial @("shell", "getprop", "ro.product.cpu.abilist")).Trim().Split(",") | Where-Object { $_ }
    $dir = "/data/local/tmp/encore-smoke-" + [guid]::NewGuid().ToString("N").Substring(0, 8)
    Write-Host "== $model ($serial), Android $release (API $sdk), ABIs $($abis -join ', ')"
    try {
        # Everything this run puts on the phone is inside $dir, removed in finally.
        Invoke-Adb $serial @("shell", "mkdir", "-p", "$dir/tmp") | Out-Null
        Invoke-Adb $serial @("push", (Join-Path $Bundle "classes.dex"), "$dir/classes.dex") | Out-Null
        foreach ($abi in $abis) {
            $libs = Join-Path $Bundle $abi
            if (-not (Test-Path (Join-Path $libs "libencore_core.so"))) {
                $results += [pscustomobject]@{ Device = $model; Serial = $serial; Android = "$release (API $sdk)"; Abi = $abi; Result = "not built (no library for this ABI)"; Seconds = 0 }
                continue
            }
            Invoke-Adb $serial @("shell", "mkdir", "-p", "$dir/$abi") | Out-Null
            foreach ($f in @("libencore_core.so", "libjnidispatch.so")) { Invoke-Adb $serial @("push", (Join-Path $libs $f), "$dir/$abi/$f") | Out-Null }
            $tag = "$serial-$abi"
            $replay = Start-Helper @((Join-Path $repo "tools\ci\replay_jellyfin.py"), $fixture) (Join-Path $work "$tag.replay")
            $tls = Start-Helper @((Join-Path $PSScriptRoot "tls_server.py"), (Join-Path $Bundle "tls\cert.pem"), (Join-Path $Bundle "tls\key.pem")) (Join-Path $work "$tag.tls")
            $rp = ([uri]$replay.url).Port; $tp = ([uri]$tls.url).Port
            try {
                Invoke-Adb $serial @("reverse", "tcp:$rp", "tcp:$rp") | Out-Null
                Invoke-Adb $serial @("reverse", "tcp:$tp", "tcp:$tp") | Out-Null
                $proc = if ($abi -match '64') { "app_process64" } else { "app_process32" }
                $cmd = "cd $dir && CLASSPATH=$dir/classes.dex LD_LIBRARY_PATH=$dir/$abi " +
                "ENCORE_REPLAY_URL=$($replay.url) ENCORE_TLS_URL=$($tls.url) " +
                "$proc -Djava.io.tmpdir=$dir/tmp -Djava.library.path=$dir/$abi -Djna.library.path=$dir/$abi " +
                "-Djna.boot.library.path=$dir/$abi $dir MainKt; echo exit=`$?"
                $t0 = Get-Date
                $output = Invoke-Adb $serial @("shell", $cmd)
                $secs = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
                Set-Content -Path (Join-Path $work "$tag.log") -Value $output -Encoding utf8
                $ok = ($output -match 'library flow OK') -and ($output -match 'https guard OK') -and ($output -match "parity $expected") -and ($output -match 'exit=0')
                $parity = if ($output -match 'parity ([0-9a-f]{8})') { $Matches[1] } else { "none" }
                $verdict = if ($ok) { "passed (parity $parity)" } else { "FAILED (parity $parity): " + (($output -split "`n" | Select-Object -Last 6) -join " / ") }
                $results += [pscustomobject]@{ Device = $model; Serial = $serial; Android = "$release (API $sdk)"; Abi = $abi; Result = $verdict; Seconds = $secs }
                Write-Host "   $abi : $verdict in $secs s"
            }
            finally {
                Invoke-Adb $serial @("reverse", "--remove", "tcp:$rp") | Out-Null
                Invoke-Adb $serial @("reverse", "--remove", "tcp:$tp") | Out-Null
                Stop-Process -Id $replay.proc.Id -Force -ErrorAction SilentlyContinue
                Stop-Process -Id $tls.proc.Id -Force -ErrorAction SilentlyContinue
            }
        }
    }
    finally {
        Invoke-Adb $serial @("shell", "rm", "-rf", $dir) | Out-Null
        $left = Invoke-Adb $serial @("shell", "ls", "-d", $dir, "2>/dev/null", "||", "echo", "gone")
        Write-Host "   cleaned up: $($left.Trim())"
    }
}

$table = $results | Format-Table -AutoSize | Out-String
Write-Host $table
if ($Out) {
    $lines = @("| Device | Android | ABI | Result | Seconds |", "| --- | --- | --- | --- | --- |")
    $lines += $results | ForEach-Object { "| $($_.Device) | $($_.Android) | $($_.Abi) | $($_.Result) | $($_.Seconds) |" }
    Set-Content -Path $Out -Value $lines -Encoding utf8
}
Write-Host "Logs: $work"
if (@($results | Where-Object { $_.Result -like "FAILED*" }).Count -gt 0) { exit 1 }

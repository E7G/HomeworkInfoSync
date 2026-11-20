[CmdletBinding()]
param(
    [ValidateSet('arm64-v8a', 'armeabi-v7a', 'x86_64')]
    [string[]] $Abi = @('arm64-v8a'),
    [string] $KeyStore,
    [string] $KeyAlias = 'homework-sync',
    [string] $OutputPath = (Join-Path $PSScriptRoot 'build/HomeworkInfoSync.apk'),
    [switch] $PackageOnly
)

$ErrorActionPreference = 'Stop'
if (!$env:ANDROID_HOME) { throw 'Set ANDROID_HOME to your Android SDK.' }
if (!$env:JAVA_HOME) { throw 'Set JAVA_HOME to JDK 17 or newer.' }
$sdk = $env:ANDROID_HOME
$java = Join-Path $env:JAVA_HOME 'bin'
$tools = Join-Path $sdk 'build-tools/35.0.0'
$androidJar = Join-Path $sdk 'platforms/android-35/android.jar'
if (!(Test-Path $androidJar) -or !(Test-Path (Join-Path $tools 'zipalign.exe'))) {
    throw 'Install the Android 35 platform and Android Build Tools 35.0.0.'
}
if (!$env:ANDROID_NDK_HOME) {
    $env:ANDROID_NDK_HOME = Join-Path $sdk 'ndk/28.2.13676358'
}

function Invoke-Step([string] $Exe, [string[]] $Arguments) {
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Exe exited with code $LASTEXITCODE." }
}

$work = Join-Path $PSScriptRoot 'build'
$jni = Join-Path $work 'jni'
$classes = Join-Path $work 'classes'
$dex = Join-Path $work 'dex'
New-Item -ItemType Directory -Force $work, $classes, $dex | Out-Null
$repo = Split-Path $PSScriptRoot -Parent
$manifest = Join-Path $PSScriptRoot 'app/AndroidManifest.xml'
$resources = Join-Path $work 'resources.zip'
$unsigned = Join-Path $work 'unsigned.apk'
$aligned = Join-Path $work 'aligned.apk'
$bytecode = Join-Path $work 'java-classes.jar'
$out = [IO.Path]::GetFullPath($OutputPath)
$assets = Join-Path $work 'assets'
New-Item -ItemType Directory -Force $assets | Out-Null
Copy-Item -LiteralPath (Join-Path $repo 'LICENSE') -Destination (Join-Path $assets 'LICENSE')

$targets = @{
    'arm64-v8a' = 'aarch64-linux-android'
    'armeabi-v7a' = 'armv7-linux-androideabi'
    'x86_64' = 'x86_64-linux-android'
}
Push-Location $repo
try {
    if (!$PackageOnly) {
        $argsNdk = @('ndk', '--platform', '26', '-o', $jni)
        foreach ($arch in $Abi) { $argsNdk += @('-t', $arch) }
        $argsNdk += @('build', '--release', '--locked', '-p', 'homework-android-jni')
        Invoke-Step 'cargo' $argsNdk
    }
    $licenses = [ordered]@{}
    foreach ($arch in $Abi) {
        $tree = & cargo metadata --locked --format-version 1 --filter-platform $targets[$arch]
        if ($LASTEXITCODE -ne 0) { throw 'Unable to read Rust dependency licenses.' }
        $tree = $tree | ConvertFrom-Json
        $packages = @{}
        $nodes = @{}
        foreach ($package in $tree.packages) { $packages[$package.id] = $package }
        foreach ($node in $tree.resolve.nodes) { $nodes[$node.id] = $node }
        $queue = [Collections.Generic.Queue[string]]::new()
        $queue.Enqueue(($tree.packages | Where-Object name -EQ 'homework-android-jni').id)
        $seen = [Collections.Generic.HashSet[string]]::new()
        while ($queue.Count) {
            $id = $queue.Dequeue()
            if (!$seen.Add($id)) { continue }
            foreach ($dep in $nodes[$id].dependencies) { $queue.Enqueue($dep) }
            $package = $packages[$id]
            if (!$package.source) { continue }
            $label = "$($package.name) $($package.version)"
            if ($licenses.Contains($label)) { continue }
            $base = Split-Path $package.manifest_path -Parent
            $files = @(Get-ChildItem -LiteralPath $base -File | Where-Object Name -Match '^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)')
            if ($package.license_file) {
                $files += Get-Item -LiteralPath (Join-Path $base $package.license_file)
            }
            $licenses[$label] = @("License: $($package.license)", "Source: $($package.repository)") +
                @($files | Sort-Object FullName -Unique | ForEach-Object {
                    '--- ' + $_.Name + " ---`n" + [IO.File]::ReadAllText($_.FullName)
                })
        }
    }
    $notices = foreach ($label in ($licenses.Keys | Sort-Object)) {
        "===== $label ====="
        $licenses[$label]
        ''
    }
    [IO.File]::WriteAllLines((Join-Path $assets 'THIRD-PARTY.txt'), [string[]] $notices, [Text.UTF8Encoding]::new($false))
} finally { Pop-Location }

$sources = @(Get-ChildItem -Path (Join-Path $PSScriptRoot 'app/java') -Filter '*.java' -Recurse |
    ForEach-Object FullName)
Invoke-Step (Join-Path $java 'javac.exe') (@('--release', '8', '-encoding', 'UTF-8', '-classpath', $androidJar, '-d', $classes) + $sources)
Invoke-Step (Join-Path $java 'jar.exe') @('--create', '--file', $bytecode, '-C', $classes, '.')
Invoke-Step (Join-Path $tools 'd8.bat') @('--release', '--min-api', '26', '--lib', $androidJar, '--output', $dex, $bytecode)
Invoke-Step (Join-Path $tools 'aapt2.exe') @('compile', '--dir', (Join-Path $PSScriptRoot 'app/res'), '-o', $resources)
Invoke-Step (Join-Path $tools 'aapt2.exe') @('link', '-I', $androidJar, '--manifest', $manifest, '-A', $assets, '-o', $unsigned, $resources)

Add-Type -AssemblyName System.IO.Compression.FileSystem
$apk = [IO.Compression.ZipFile]::Open($unsigned, [IO.Compression.ZipArchiveMode]::Update)
try {
    foreach ($file in (Get-ChildItem -Path $dex -Filter '*.dex')) {
        $null = [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $apk, $file.FullName, $file.Name, [IO.Compression.CompressionLevel]::Optimal)
    }
    foreach ($arch in ($Abi | Select-Object -Unique)) {
        $lib = Join-Path $jni "$arch/libhomework_android.so"
        $null = [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $apk, $lib, "lib/$arch/libhomework_android.so", [IO.Compression.CompressionLevel]::NoCompression)
    }
} finally { $apk.Dispose() }

Invoke-Step (Join-Path $tools 'zipalign.exe') @('-P', '16', '-f', '4', $unsigned, $aligned)
$createdPassword = $false
if (!$KeyStore) {
    $KeyStore = Join-Path $work 'local-signing.p12'
    if (!$env:HOMEWORK_KS_PASS) {
        $env:HOMEWORK_KS_PASS = 'local-build-only'
        $createdPassword = $true
    }
} elseif (!$env:HOMEWORK_KS_PASS) {
    throw 'Set HOMEWORK_KS_PASS for the specified signing key.'
}
try {
    if (!(Test-Path -LiteralPath $KeyStore)) {
        Invoke-Step (Join-Path $java 'keytool.exe') @('-genkeypair', '-keystore', $KeyStore, '-storetype',
            'PKCS12', '-storepass:env', 'HOMEWORK_KS_PASS', '-alias', $KeyAlias, '-keyalg', 'RSA',
            '-keysize', '3072', '-validity', '10000', '-dname', 'CN=HomeworkSync Android')
    }
    New-Item -ItemType Directory -Force (Split-Path $out -Parent) | Out-Null
    Invoke-Step (Join-Path $tools 'apksigner.bat') @('sign', '--ks', $KeyStore, '--ks-key-alias', $KeyAlias,
        '--ks-pass', 'env:HOMEWORK_KS_PASS', '--v4-signing-enabled', 'false', '--out', $out, $aligned)
} finally {
    if ($createdPassword) { Remove-Item Env:HOMEWORK_KS_PASS }
}
Invoke-Step (Join-Path $tools 'zipalign.exe') @('-c', '-P', '16', '4', $out)
Invoke-Step (Join-Path $tools 'apksigner.bat') @('verify', '--verbose', $out)
Get-Item -LiteralPath $out

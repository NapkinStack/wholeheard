# D54's second criterion, on GitHub's Windows runner: the package signed with a certificate made for
# this run, installed, its process started with its window, upgraded and removed. No device sounds
# here: a meeting folder is placed, nothing is recorded. The private key never leaves this machine's
# store, and goes with it; the public certificate stays beside the package, for a colleague to trust.
param([Parameter(Mandatory)][string]$Artefacts, [Parameter(Mandatory)][string]$Out)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot

function Check([string]$What, [bool]$Held) {
    Write-Output "  $(if ($Held) { 'ok  ' } else { 'FAIL' })  $What"
    if (-not $Held) { throw "trial: $What" }
}
function Wait-For([string]$What, [int]$Seconds, [scriptblock]$Until) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        $got = & $Until
        if ($got) { return $got }
        Start-Sleep -Seconds 2
    }
    throw "trial: $What, not within $Seconds s"
}

$identity = Get-Content (Join-Path $here 'identity.json') -Raw | ConvertFrom-Json
if ($identity.placeholder) { Write-Output "The identity is a placeholder: signed for this run, not submitted." }
$version = (Get-Content (Join-Path $Artefacts 'shell/VERSION') -Raw).Trim()
$first = Join-Path $Out "Wholeheard-$version-x64.msix"
$parts = $version.Split('.')
$later = "$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"
& python (Join-Path $here 'msix.py') $Artefacts $later $env:RUNNER_TEMP | Out-Host
if ($LASTEXITCODE) { throw "trial: the later version was not made" }
$second = Join-Path $env:RUNNER_TEMP "Wholeheard-$later-x64.msix"

$certificate = New-SelfSignedCertificate -Type Custom -KeyUsage DigitalSignature `
    -CertStoreLocation 'Cert:\CurrentUser\My' -NotAfter (Get-Date).AddDays(30) `
    -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}') `
    -Subject $identity.publisher -FriendlyName 'Wholeheard, this run only'
$public = Join-Path $Out 'Wholeheard-this-run.cer'
Export-Certificate -Cert $certificate -FilePath $public | Out-Null
Import-Certificate -FilePath $public -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null
try {
    $signtool = (Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\10.*\x64\signtool.exe' |
        Sort-Object FullName | Select-Object -Last 1).FullName
    foreach ($package in $first, $second) {
        & $signtool sign /fd SHA256 /sha1 $certificate.Thumbprint $package | Out-Host
        if ($LASTEXITCODE) { throw "trial: $package was not signed" }
    }

    function Start-Wholeheard($Installed) {
        Start-Process "shell:AppsFolder\$($Installed.PackageFamilyName)!Wholeheard"
        $process = Wait-For 'its process with a window, from the package' 90 {
            Get-Process wholeheard -ErrorAction SilentlyContinue |
                Where-Object { $_.MainWindowHandle -ne 0 -and $_.Path -like "$($Installed.InstallLocation)*" } |
                Select-Object -First 1
        }
        Check "the process runs from $($Installed.InstallLocation), its window open: $($process.MainWindowTitle)" $true
        $line = (Get-CimInstance Win32_Process -Filter "ProcessId = $($process.Id)").CommandLine
        Check "started with --window: $line" ($line -like '*--window*')
        Stop-Process -Id $process.Id -Force
        Wait-For 'its process gone' 30 { -not (Get-Process wholeheard -ErrorAction SilentlyContinue) } | Out-Null
    }

    Add-AppxPackage -Path $first
    $installed = Get-AppxPackage -Name $identity.name
    Check "installed, version $version.0" ($installed.Version -eq "$version.0")
    foreach ($program in 'wholeheard', 'wholeheard-capture', 'wholeheard-transcription', 'wholeheard-summary') {
        Check "$program.exe inside it" (Test-Path (Join-Path $installed.InstallLocation "$program.exe"))
    }
    $own = Join-Path $env:LOCALAPPDATA "Packages\$($installed.PackageFamilyName)\LocalCache"
    try {
        Start-Wholeheard $installed
    } finally {
        if (Test-Path $own) { Get-ChildItem $own -Recurse | ForEach-Object { Write-Output $_.FullName } }
    }
    Check "its logs are in the package's own data, as Windows redirects them" (Test-Path (Join-Path $own 'Local\Wholeheard\logs'))

    $meeting = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Wholeheard\trial-meeting'
    New-Item -ItemType Directory -Force $meeting | Out-Null
    Set-Content (Join-Path $meeting 'transcript.md') 'A meeting placed by the trial.'
    $settings = Join-Path $own 'Roaming\wholeheard\settings.toml'
    New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
    Set-Content $settings 'welcomed = true'

    Add-AppxPackage -Path $second
    $installed = Get-AppxPackage -Name $identity.name
    Check "upgraded over it, version $later.0" ($installed.Version -eq "$later.0")
    Check 'the meeting folder is still there' (Test-Path (Join-Path $meeting 'transcript.md'))
    Check 'the settings are still there' (Test-Path $settings)
    Start-Wholeheard $installed

    Remove-AppxPackage -Package $installed.PackageFullName
    Check 'removed' (-not (Get-AppxPackage -Name $identity.name))
    Check 'the meeting folder is still there' (Test-Path (Join-Path $meeting 'transcript.md'))
    Wait-For 'the settings gone with the package' 60 { -not (Test-Path $settings) } | Out-Null
    Check 'the settings went with it: certification.md says so' $true
} finally {
    Remove-Item "Cert:\LocalMachine\TrustedPeople\$($certificate.Thumbprint)" -ErrorAction SilentlyContinue
    Remove-Item "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -ErrorAction SilentlyContinue
}
Write-Output 'trial: passed'

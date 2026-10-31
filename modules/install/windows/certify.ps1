# D77: the Windows App Certification Kit tests the package before the Store does. Its report is
# written beside the package, and each failed test is named in the run (appcert.py). A kit that
# cannot run here is said in one line, and the run goes on: the kit's verdict informs, it blocks
# nothing — the identity is a placeholder and the version starts at 0 until a submission.
param([Parameter(Mandatory)][string]$Out)
$ErrorActionPreference = 'Stop'

$kit = 'C:\Program Files (x86)\Windows Kits\10\App Certification Kit\appcert.exe'
if (-not (Test-Path $kit)) {
    Write-Output "certification kit: not run — appcert.exe is not on this machine ($kit)"
    exit 0
}
$package = (Get-ChildItem (Join-Path $Out 'Wholeheard-*-x64.msix') | Select-Object -First 1).FullName
$report = Join-Path (Resolve-Path $Out) 'appcert-report.xml'
# The kit's return codes, as `appcert.exe /?` lists them.
$meaning = @{ -1 = 'invalid command line'; -2 = 'infrastructure error'; -3 = 'user initiated error'
              -4 = 'app installation error'; -5 = 'app unpackaging error' }

$certificate = Import-Certificate -FilePath (Join-Path $Out 'Wholeheard-this-run.cer') `
    -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople'
try {
    & $kit reset | Out-Host
    & $kit test -appxpackagepath $package -reportoutputpath $report | Out-Host
    $code = $LASTEXITCODE
} finally {
    Remove-Item "Cert:\LocalMachine\TrustedPeople\$($certificate.Thumbprint)" -ErrorAction SilentlyContinue
}
if ($code -notin 0, 1 -and -not (Test-Path $report)) {
    Write-Output "certification kit: not run — appcert.exe test ended with $code ($($meaning[$code]))"
    exit 0
}
& python (Join-Path $PSScriptRoot 'appcert.py') $report
exit 0

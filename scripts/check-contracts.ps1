$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Join-Path $PSScriptRoot '..')
$paths = @('packages/identity-api-types/src/generated.ts', 'wallet/src/bridge/generated.ts')
$before = @{}
foreach ($path in $paths) { $before[$path] = [IO.File]::ReadAllText((Join-Path (Get-Location) $path)) }
& cargo run -p crypto-sso-identity-api --example export_types
if ($LASTEXITCODE -ne 0) { throw 'HTTP contract generation failed' }
& cargo run -p wallet --example export_ipc
if ($LASTEXITCODE -ne 0) { throw 'IPC contract generation failed' }
foreach ($path in $paths) {
    if ([IO.File]::ReadAllText((Join-Path (Get-Location) $path)) -cne $before[$path]) { throw "Generated contract changed: $path. Review the generated file." }
}
Write-Host 'HTTP and IPC contracts are current.'

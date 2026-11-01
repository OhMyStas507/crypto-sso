$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Join-Path $PSScriptRoot '..')
$paths = @('packages/identity-api-types/src/generated.ts', 'wallet/src/bridge/generated.ts')
$before = @{}
# Windows 检出可能使用 CRLF；只忽略换行格式，继续严格比较生成的类型内容。
foreach ($path in $paths) { $before[$path] = [IO.File]::ReadAllText((Join-Path (Get-Location) $path)).Replace("`r`n", "`n") }
& cargo run -p crypto-sso-identity-api --example export_types
if ($LASTEXITCODE -ne 0) { throw 'HTTP contract generation failed' }
& cargo run -p wallet --example export_ipc
if ($LASTEXITCODE -ne 0) { throw 'IPC contract generation failed' }
foreach ($path in $paths) {
    if ([IO.File]::ReadAllText((Join-Path (Get-Location) $path)).Replace("`r`n", "`n") -cne $before[$path]) { throw "Generated contract changed: $path. Review the generated file." }
}
Write-Host 'HTTP and IPC contracts are current.'

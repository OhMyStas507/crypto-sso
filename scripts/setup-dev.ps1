param([string]$Organization = '本地开发身份中心')
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $workspace
if (Test-Path -LiteralPath '.env') { throw '.env already exists; preserve your current configuration.' }
New-Item -ItemType Directory -Path '.dev-data' -Force | Out-Null
function New-HexToken([int]$Length) {
    $bytes = [byte[]]::new($Length)
    $random = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try { $random.GetBytes($bytes) } finally { $random.Dispose() }
    return ([BitConverter]::ToString($bytes)).Replace('-', '').ToLowerInvariant()
}
if (-not (Test-Path -LiteralPath '.dev-data/issuer-secret.json')) {
    $publicJson = & cargo run -p utils --bin generate-issuer-key -- --organization $Organization --output .dev-data/issuer-secret.json
    if ($LASTEXITCODE -ne 0) { throw 'Issuer key generation failed' }
    [IO.File]::WriteAllText((Join-Path $workspace '.dev-data/issuer-public.json'), ($publicJson -join "`n"), [Text.UTF8Encoding]::new($false))
}
$password = New-HexToken 16
$env:ADMIN_BOOTSTRAP_PASSWORD = $password
try { $passwordHash = & cargo run -p issuer-service --bin hash-password } finally { Remove-Item Env:ADMIN_BOOTSTRAP_PASSWORD }
if ($LASTEXITCODE -ne 0) { throw 'Password hashing failed' }
$content = Get-Content -LiteralPath '.env.example' -Raw
$content = $content.Replace('REPLACE_WITH_ARGON2_HASH', ($passwordHash -join '').Trim())
$content = $content.Replace('REPLACE_WITH_64_OR_MORE_RANDOM_BYTES', (New-HexToken 64))
$content = $content.Replace('REPLACE_WITH_64_HEX_CHARACTERS', (New-HexToken 32))
$content = $content.Replace('REPLACE_WITH_A_DIFFERENT_64_BYTE_SECRET', (New-HexToken 64))
$content = $content.Replace('REPLACE_WITH_A_DIFFERENT_64_HEX_KEY', (New-HexToken 32))
foreach ($relative in @('.dev-data/issuer-secret.json', '.dev-data/issuer-public.json', '.dev-data/wallet')) {
    $absolute = (Join-Path $workspace $relative).Replace('\', '/')
    $content = $content.Replace("=$relative", "='$absolute'")
}
[IO.File]::WriteAllText((Join-Path $workspace '.env'), $content, [Text.UTF8Encoding]::new($false))
Write-Host "Created .env. Development admin: admin / $password"
Write-Host 'Keep this generated password private. It is not saved in plaintext.'

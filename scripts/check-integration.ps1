$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Join-Path $PSScriptRoot '..')
function Check([scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw 'A required check failed.' }
}
Check { cargo fmt -p issuer-service -p wallet -p crypto-sso-identity-api -p crypto-sso-relying-party -p crypto-sso-telemetry -- --check }
Check { cargo clippy -p issuer-service -p wallet -p crypto-sso-identity-api -p crypto-sso-relying-party -p crypto-sso-telemetry --all-targets -- -D warnings }
Check { cargo test -p wallet --lib }
Check { cargo test -p issuer-service --test access_logs }
Check { cargo test -p issuer-service --test migrations -- --ignored }
Check { cargo test -p issuer-service --test transactions -- --ignored }
Check { cargo test -p crypto-sso-relying-party --test end_to_end -- --ignored }
Check { bun run --cwd issuer-service-webui build }
Check { bun run --cwd wallet build }
Check { bun run --cwd examples/relying-party/web build }
& "$PSScriptRoot/check-contracts.ps1"
Check { node scripts/check-ui.mjs }

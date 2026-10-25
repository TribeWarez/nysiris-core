<#
.SYNOPSIS
    Push the GitHub Actions secrets and variables the Nysiris release
    workflows require.

.DESCRIPTION
    Configures three repositories:

      TribeWarez/nysiris-core   CARGO_REGISTRY_TOKEN (secret)
                                CORE_PUBLISH_ENABLED (variable)
                                crates-io environment
      odelyzid/Nysiris          ANDROID_KEYSTORE_BASE64 (secret)
                                ANDROID_KEYSTORE_PASSWORD (secret)
                                ANDROID_KEY_ALIAS (secret)
                                ANDROID_KEY_PASSWORD (secret)
      TribeWarez/ncc            nothing required (public submodule, GITHUB_TOKEN)

    Requires an authenticated GitHub CLI with admin rights on those repos:

        gh auth login
        gh auth status

    Secret values are written only to short-lived temp files that are deleted
    immediately, and are never echoed. The crates.io token defaults to
    $env:CARGO_REGISTRY_TOKEN so it does not have to be passed on the command
    line (which would end up in shell history).

.EXAMPLE
    $env:CARGO_REGISTRY_TOKEN = '...'
    ./scripts/configure-github.ps1 -EnablePublish -KeystorePath ../Nysiris/nysiris-upload.keystore

.EXAMPLE
    # Configure only the Core publishing variable/token.
    ./scripts/configure-github.ps1 -SkipAndroid
#>
[CmdletBinding()]
param(
    [string]$CargoRegistryToken = $env:CARGO_REGISTRY_TOKEN,
    [string]$KeystorePath = $env:NYSIRIS_KEYSTORE,
    [switch]$EnablePublish,
    [switch]$SkipAndroid,
    [string]$CoreRepo = 'TribeWarez/nysiris-core',
    [string]$AppRepo = 'odelyzid/Nysiris',
    [string]$NccRepo = 'TribeWarez/ncc'
)

$ErrorActionPreference = 'Stop'

function Assert-Gh {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw "GitHub CLI (gh) is not installed. Install it from https://cli.github.com/ and run: gh auth login"
    }
    & gh auth status *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "gh is not authenticated. Run: gh auth login"
    }
    & gh auth status
}

function Set-GhSecretFromText {
    param(
        [Parameter(Mandatory = $true)][string]$Repo,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value
    )
    # --body-file keeps the value out of the process command line.
    $tmp = [IO.Path]::GetTempFileName()
    try {
        [IO.File]::WriteAllText($tmp, $Value, (New-Object Text.UTF8Encoding($false)))
        & gh secret set $Name --repo $Repo --body-file $tmp
        if ($LASTEXITCODE -ne 0) { throw "gh secret set $Name (--repo $Repo) failed" }
    }
    finally {
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    }
}

function Read-SecretText {
    param([Parameter(Mandatory = $true)][string]$Prompt)
    $secure = Read-Host -Prompt $Prompt -AsSecureString
    $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    try {
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    }
    finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
    }
}

Assert-Gh

# --- nysiris-core: crates.io publishing -------------------------------------

if ($CargoRegistryToken) {
    Set-GhSecretFromText -Repo $CoreRepo -Name 'CARGO_REGISTRY_TOKEN' -Value $CargoRegistryToken
    Write-Host "set CARGO_REGISTRY_TOKEN on $CoreRepo"
}
else {
    Write-Warning "CARGO_REGISTRY_TOKEN not supplied; crates.io publishing stays impossible."
    Write-Warning "Re-run with `$env:CARGO_REGISTRY_TOKEN set, or run: gh secret set CARGO_REGISTRY_TOKEN --repo $CoreRepo"
}

$publishValue = if ($EnablePublish) { 'true' } else { 'false' }
& gh variable set CORE_PUBLISH_ENABLED --repo $CoreRepo --body $publishValue
if ($LASTEXITCODE -ne 0) { throw "gh variable set CORE_PUBLISH_ENABLED (--repo $CoreRepo) failed" }
Write-Host "set CORE_PUBLISH_ENABLED=$publishValue on $CoreRepo"

# The workflow declares `environment: crates-io`; creating it explicitly lets
# you attach required reviewers or environment-scoped secrets later.
& gh api --method PUT "repos/$CoreRepo/environments/crates-io" *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Warning "could not create the crates-io environment; GitHub will create it on first use"
}
else {
    Write-Host "ensured environment crates-io on $CoreRepo"
}

# --- Nysiris: Android release signing ---------------------------------------

if ($SkipAndroid) {
    Write-Host "skipping Android signing secrets"
}
else {
    if (-not $KeystorePath) {
        $KeystorePath = Read-Host -Prompt "Path to the Android keystore (e.g. G:\projects\Nysiris\nysiris-upload.keystore)"
    }
    if (-not (Test-Path -LiteralPath $KeystorePath)) {
        throw "keystore not found: $KeystorePath"
    }
    $resolvedKeystore = (Resolve-Path -LiteralPath $KeystorePath).Path
    $base64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes($resolvedKeystore))
    Set-GhSecretFromText -Repo $AppRepo -Name 'ANDROID_KEYSTORE_BASE64' -Value $base64
    Write-Host "set ANDROID_KEYSTORE_BASE64 on $AppRepo from $resolvedKeystore"

    $storePassword = Read-SecretText -Prompt 'Keystore password'
    if (-not $storePassword) { throw 'keystore password is required when a keystore is configured' }
    Set-GhSecretFromText -Repo $AppRepo -Name 'ANDROID_KEYSTORE_PASSWORD' -Value $storePassword
    Write-Host "set ANDROID_KEYSTORE_PASSWORD on $AppRepo"

    $alias = Read-Host -Prompt 'Key alias [nysiris]'
    if (-not $alias) { $alias = 'nysiris' }
    Set-GhSecretFromText -Repo $AppRepo -Name 'ANDROID_KEY_ALIAS' -Value $alias
    Write-Host "set ANDROID_KEY_ALIAS on $AppRepo"

    $keyPassword = Read-SecretText -Prompt 'Key password (blank = same as keystore)'
    if (-not $keyPassword) { $keyPassword = $storePassword }
    Set-GhSecretFromText -Repo $AppRepo -Name 'ANDROID_KEY_PASSWORD' -Value $keyPassword
    Write-Host "set ANDROID_KEY_PASSWORD on $AppRepo"
}

# --- Verify -----------------------------------------------------------------

Write-Host ''
Write-Host "Configured. Verify with:"
Write-Host "  gh secret list --repo $CoreRepo"
Write-Host "  gh variable list --repo $CoreRepo"
Write-Host "  gh secret list --repo $AppRepo"
Write-Host "  gh secret list --repo $NccRepo   # expected: empty"
Write-Host ''
Write-Host "Then cut a Core release (v0.1.0 is already tagged, so use the next version):"
Write-Host "  1. bump [workspace.package].version (and the workspace dependency versions)"
Write-Host "  2. commit and push"
Write-Host "  3. git tag vX.Y.Z && git push origin vX.Y.Z"

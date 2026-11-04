<# Exercise the release signature gate with hermetic unsigned, wrong-identity and missing-timestamp fixtures. #>
$ErrorActionPreference = 'Stop'
$Fixture = New-TemporaryFile
$State = [pscustomobject]@{ Signature = $null }
$PreviousReader = Get-Item -LiteralPath Function:\Get-AuthenticodeSignature -ErrorAction SilentlyContinue
try {
  # A closure keeps fixture state bound to this test when the verifier runs in a child script scope.
  $Reader = { param([string]$LiteralPath) return $State.Signature }.GetNewClosure()
  Set-Item -LiteralPath Function:\Get-AuthenticodeSignature -Value $Reader
  $Cases = @(
    @{ Name = 'unsigned'; Status = 'NotSigned'; Subject = 'CN=Fixture'; Timestamp = $true; Reject = $true },
    @{ Name = 'wrong identity'; Status = 'Valid'; Subject = 'CN=Wrong'; Timestamp = $true; Reject = $true },
    @{ Name = 'missing timestamp'; Status = 'Valid'; Subject = 'CN=Fixture'; Timestamp = $false; Reject = $true },
    @{ Name = 'valid timestamped identity'; Status = 'Valid'; Subject = 'CN=Fixture'; Timestamp = $true; Reject = $false }
  )
  foreach ($Case in $Cases) {
    $State.Signature = [pscustomobject]@{
      Status = $Case.Status
      SignerCertificate = [pscustomobject]@{ Subject = $Case.Subject }
      TimeStamperCertificate = $(if ($Case.Timestamp) { [pscustomobject]@{ Subject = 'CN=Fixture timestamp' } } else { $null })
    }
    $Rejected = $false
    $Reason = 'The guard accepted the fixture.'
    try { & "$PSScriptRoot/verify-windows-signatures.ps1" -Files @($Fixture.FullName) -ExpectedSubject 'CN=Fixture' }
    catch { $Rejected = $true; $Reason = $_.Exception.Message }
    if ($Rejected -ne $Case.Reject) { throw "Signature fixture '$($Case.Name)' produced the wrong verdict: $Reason" }
    Write-Host "PASS signature fixture: $($Case.Name)"
  }
  Write-Host 'PASS signature validation: unsigned, wrong identity, missing timestamp and valid fixture.'
} finally {
  if ($null -ne $PreviousReader) { Set-Item -LiteralPath Function:\Get-AuthenticodeSignature -Value $PreviousReader.ScriptBlock }
  else { Remove-Item -LiteralPath Function:\Get-AuthenticodeSignature -ErrorAction SilentlyContinue }
  Remove-Item -LiteralPath $Fixture.FullName -Force
}

# 0005. Terraform w repo, wdrażanie z IDE przez SSO

**Status:** przyjęta (2026-10)

## Kontekst

Projekt hobbystyczny/CV z jednym autorem. Infrastruktura ma być w całości kodem w tym repo i dać się wdrożyć jednym poleceniem z IDE (VS Code na Windows). Długotrwałe klucze dostępu AWS to ryzyko.

## Decyzja

- **Terraform** (≥ 1.10): `infra/bootstrap` (stan lokalny – bucket na stan i budżety), `infra/envs/prod` (stan w S3, blokada `use_lockfile` bez DynamoDB), moduły w `infra/modules`. Kolejne środowisko (staging) = nowy katalog w `envs/` z tymi samymi modułami.
- **Uwierzytelnienie:** IAM Identity Center i `aws sso login --profile wieczko` – krótkotrwałe poświadczenia, brak kluczy na dysku.
- **Wdrażanie z IDE:** zadania VS Code wołające skrypty `tools/deploy/` (Node, jak reszta `tools/`). Kod Lambdy wdraża `terraform apply` przez `tools/deploy/infra.mjs` (build `cargo zigbuild`, zip `archive_file`, `source_code_hash`); frontend – `aws s3 sync` + inwalidacja; game-server – binarka do S3 + `ssm send-command`.
- **CI (GitHub Actions)** tylko weryfikuje (testy, buildy, `terraform validate`) i nie ma dostępu do AWS. Wdrażanie z CI (rola OIDC) – opcjonalnie później.

## Konsekwencje

- Wdrożenie wymaga lokalnych narzędzi (Terraform, AWS CLI, Zig + cargo-zigbuild) – opisane w `infra/README.md`.
- Brak automatycznego wdrożenia po merge – świadomie, przy jednym autorze to prostsze i bezpieczniejsze.

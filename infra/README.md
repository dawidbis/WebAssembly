# Infrastruktura AWS (Terraform)

Cała infrastruktura gry jest tutaj jako kod i wdrażana z IDE (zadania VS Code, później `tools/deploy/`). Decyzje architektoniczne: [docs/adr/](../docs/adr/).

## Architektura docelowa

```
                  https://<domena>  (Route 53 → CloudFront, certyfikat ACM z us-east-1)
                                   │
                          CloudFront (always free: 1 TB, 10 mln żądań)
          ┌────────────────────────┼─────────────────────────────┐
     /*  (domyślne)            /api/*                          /ws*  (WebSocket, bez cache)
          │                        │                              │
   S3 (frontend, OAC)      API Gateway HTTP API         EC2 t4g.micro – game-server (Rust)
   + CloudFront Function           │                     SG: tylko prefix list CloudFront
     (fallback SPA)        Lambda `meta` (Rust, arm64)   + nagłówek X-Origin-Verify
                                   │                              │
                           DynamoDB (tabela pokoi) ◄──────────────┘ heartbeat pokoi
                                   │
                           SSM Parameter Store: klucz Ed25519 do biletów dołączenia
```

Jedna domena dla wszystkiego: brak CORS, jeden certyfikat, TLS kończy CloudFront (EC2 nie potrzebuje certyfikatu).

## Struktura

| Katalog | Co | Stan Terraform |
|---|---|---|
| `bootstrap/` | bucket na stan, budżety (alarmy e-mail) | lokalny, uruchamiany raz |
| `envs/prod/` | środowisko produkcyjne – wywołania modułów | S3 (`backend.hcl`) |
| `modules/dns` | opcjonalna własna domena: strefa Route 53, certyfikat ACM (us-east-1) | – |
| `modules/frontend` | bucket S3 + CloudFront (OAC, fallback SPA, nagłówki bezpieczeństwa); opcjonalne zachowania `/api/*` i `/ws*` | – |
| `modules/game_server` | EC2 t4g.micro (AL2023 arm64) z game-serverem: SG tylko z CloudFront, IAM, sekret originu w SSM, systemd, logi CloudWatch, alarm `recover`, bucket artefaktów | – |
| `modules/meta` | dochodzi w etapie lobby | – |

Pliki z danymi konta (`terraform.tfvars`, `backend.hcl`) są poza repo – w repo są ich wzory `*.example`.

## Wymagania lokalne (Windows / Linux)

| Narzędzie | Wersja | Instalacja (Windows) |
|---|---|---|
| Terraform | ≥ 1.10 (blokady stanu w S3 bez DynamoDB) | `winget install Hashicorp.Terraform` |
| AWS CLI | v2 | `winget install Amazon.AWSCLI` |
| cargo-lambda (etap meta) | aktualny | `pip install cargo-lambda` albo `winget install CargoLambda.CargoLambda` |
| Zig + cargo-zigbuild (game-server) | aktualny | `winget install zig.zig`, `cargo install cargo-zigbuild`, `rustup target add aarch64-unknown-linux-musl`; po instalacji Ziga uruchom ponownie VS Code (nowy PATH) |

## Krok 0 – konto (ręcznie, raz)

1. **Root:** włącz MFA, nie używaj roota do pracy.
2. **IAM Identity Center** (region `eu-central-1`): włącz, utwórz użytkownika, przypisz go do konta z permission set `AdministratorAccess` (projekt jednoosobowy; później można zawęzić).
3. **Profil SSO na laptopie:**
   ```bash
   aws configure sso --profile wieczko      # SSO start URL z Identity Center, region eu-central-1
   aws sso login --profile wieczko
   aws sts get-caller-identity --profile wieczko
   ```
4. **Domena (opcjonalnie, można później):** bez niej gra działa pod `https://<id>.cloudfront.net`. Kupiona w Route 53 – strefa powstaje sama, ustaw `domain_name` (`create_zone = false`). U innego rejestratora – ustaw też `create_zone = true`, a serwery NS z `terraform output name_servers` wpisz u rejestratora.

## Krok 1 – bootstrap (raz)

```bash
cd infra/bootstrap
cp terraform.tfvars.example terraform.tfvars   # uzupełnij budget_email
terraform init
terraform apply
terraform output -raw backend_hcl > ../envs/prod/backend.hcl
# PowerShell: terraform output -raw backend_hcl | Out-File -Encoding ascii ..\envs\prod\backend.hcl
```

Tworzy bucket `mapa-tfstate-<konto>` (wersjonowanie, szyfrowanie, tylko TLS, blokada usunięcia, logi dostępu), bucket `mapa-logs-<konto>` (logi dostępu S3 i CloudFront wszystkich środowisk, wygasają po 30 dniach) i dwa budżety miesięczne (1 i 10 USD, alarm faktyczny i prognozowany). Budżety liczą zużycie **przed kredytami** – inaczej na Free plan pokazywałyby 0 USD aż do wyczerpania kredytów. Alarmy przychodzą e-mailem bez potwierdzania subskrypcji.

Stan bootstrapu (`infra/bootstrap/terraform.tfstate`) zostaje lokalnie – nie usuwaj go (albo zaimportuj zasoby ponownie przez `terraform import`).

## Krok 2 – środowisko prod

```bash
cd infra/envs/prod
cp terraform.tfvars.example terraform.tfvars   # opcjonalnie: domain_name
terraform init "-backend-config=backend.hcl"   # cudzysłów wymagany w PowerShell
terraform plan
terraform apply
terraform output url                           # https://<id>.cloudfront.net (albo własna domena)
```

Albo z VS Code: **Terminal → Run Task…** → „AWS: login (SSO)”, „AWS: init”, „AWS: plan”, „AWS: apply (infrastruktura)” (`.vscode/tasks.json`).

## Krok 3 – wdrożenie frontendu

```bash
node tools/deploy/frontend.mjs             # build produkcyjny + upload + inwalidacja
node tools/deploy/frontend.mjs --prep      # po zmianach w Ruście (typy + wasm)
node tools/deploy/frontend.mjs --skip-build --dry-run   # tylko pokaż, co i z jakim cache pójdzie
```

Zadania VS Code: „AWS: deploy frontend”. Profil AWS ze zmiennej `AWS_PROFILE` (domyślnie `wieczko`).

| Pliki | Cache-Control | Dlaczego |
|---|---|---|
| z hashem w nazwie (`main-XXXX.js`, `chunk-*.js`, `worker-*.js`, `styles-*.css`) | `public,max-age=31536000,immutable` | nowa treść = nowa nazwa |
| `index.html`, `wasm/game_wasm_bg.wasm`, `favicon.ico` | `no-cache` | stała nazwa – rewalidacja ETagiem (304) |

Content-Type jest ustawiany jawnie (AWS CLI na Windows zgaduje go z rejestru – bywa `text/plain` dla `.js`). Bez game-servera strona działa w trybie offline (mapa z parametrów domyślnych).

**Sprawdzenie po wdrożeniu:**

```bash
URL=$(terraform -chdir=infra/envs/prod output -raw url)
curl -sI $URL/ | grep -iE 'cache-control|content-type|strict-transport'
curl -sI $URL/jakas/trasa | head -1                       # 200 – fallback SPA
curl -s -o /dev/null -D - -H 'Accept-Encoding: br' $URL/wasm/game_wasm_bg.wasm | grep -iE 'content-(type|encoding)|etag'   # GET, nie -I
```

CloudFront kompresuje `application/wasm` sam (brotli, sprawdzone: `content-encoding: br`, słaby ETag `W/...`). Sprawdzaj zwykłym GET-em – odpowiedź na HEAD (`curl -I`) nie ma treści, więc nie jest kompresowana. W PowerShell: `curl.exe` (samo `curl` to alias `Invoke-WebRequest`) i `-o NUL`.

## Krok 4 – game-server (EC2)

`terraform apply` tworzy instancję, a cloud-init instaluje na niej agenta CloudWatch, unit systemd i skrypt startowy (sekret originu z SSM). Binarki jeszcze nie ma – usługa czeka (`ConditionPathExists`). Wgrywa ją skrypt:

```bash
node tools/deploy/game-server.mjs        # zigbuild (Linux arm64, musl) → S3 → SSM: podmiana + restart + /health
```

Zadania VS Code: „AWS: deploy game-server”, „AWS: logi game-servera (na żywo)”.

- Restart kończy trwające gry (pokoje są w pamięci procesu); klienci łączą się ponownie sami (`Transport` co 5 s) i zaczynają grę od nowa na tej samej mapie.
- Poprzednie binarki leżą w `s3://<artifacts>/game-server/builds/<commit>` (30 dni).
- Dostęp do instancji (bez SSH): `aws ssm start-session --target <instance-id> --profile wieczko` (wymaga wtyczki Session Manager do AWS CLI), logi: `journalctl -u game-server`, `/var/log/game-server/server.log`.
- Do czasu etapu lobby serwer działa w **trybie otwartym** (jeden pokój `default`, mapa z parametrów domyślnych) – bez biletów. Klucz publiczny biletów dojdzie jako parametr SSM `/mapa/prod/ticket-public-key`; skrypt startowy włączy wtedy bilety po restarcie.
- **Stop/start instancji zmienia jej publiczną nazwę DNS** (origin CloudFront) – po starcie uruchom `terraform apply`. Zatrzymana instancja nie kosztuje za CPU (dysk i adres IPv4 – tak); frontend działa wtedy offline.

**Sprawdzenie po wdrożeniu:**

```bash
curl -s -o /dev/null -w '%{http_code}\n' $URL/ws                 # 400 – CloudFront przekazał do serwera (brak nagłówków WebSocket)
curl -s -m 5 http://<public-dns>:3000/health || echo zablokowane   # z internetu: timeout (SG tylko dla CloudFront)
node tools/lockstep/two-tabs.mjs $URL/ --seconds 30 --delay 8 --tamper   # CHROME=ścieżka do chrome.exe na Windows
```

## Koszty (szacunek; sprawdzaj w Billing → Free Tier / Credits)

| Pozycja | Koszt/mies. | Uwagi |
|---|---|---|
| CloudFront, Lambda, DynamoDB (provisioned ≤ 25 RCU/WCU), SSM Standard, CloudWatch (5 GB logów), 2 budżety | 0 | always free |
| S3 (frontend + artefakty), API Gateway HTTP API (1 USD / 1 mln żądań) | < 0,10 USD | z kredytów |
| EC2 t4g.micro + EBS gp3 8 GB + publiczny IPv4 | ~6,1 + 0,64 + 3,65 USD | z kredytów; zatrzymana instancja = frontend w trybie offline |
| Route 53: strefa + domena | 0,50 USD + opłata roczna za domenę | poza kredytami |

Razem ok. 12 USD/mies. – kredyty Free plan (100–200 USD, 6 miesięcy) wystarczą na cały okres. Potem trzeba przejść na plan płatny.

## Bezpieczeństwo i SonarCloud

- Wszystkie buckety: prywatne, szyfrowane, polityka „tylko HTTPS”, logi dostępu do `mapa-logs-<konto>`; CloudFront zapisuje tam standardowe logi (`cloudfront/`).
- Game-server: SG tylko z prefix list CloudFront, nagłówek `X-Origin-Verify`, IMDSv2, bez SSH, sekrety w SSM.

Zgłoszenia SonarCloud zaakceptowane świadomie (oznaczone w SonarCloud jako *Accepted* / *False positive* z tym uzasadnieniem):

| Reguła | Gdzie | Dlaczego |
|---|---|---|
| `terraform:S6329` (publiczny IP) | `modules/game_server` – `associate_public_ip_address` | koszt – docs/adr/0001; dostęp tylko z CloudFront + sekretny nagłówek |
| `terraform:S6258` (brak logów) | `bootstrap` – bucket `logs` | to bucket logów; logowanie do samego siebie tworzyłoby pętlę |
| `jssecurity:S8480` | `tools/cdp.mjs` | narzędzie testowe łączy się z lokalnie uruchomionym Chrome; adres składany z naszych stałych, z odpowiedzi tylko zweryfikowany GUID (fałszywy alarm) |

## Plan wdrożenia (etapy, każdy na osobnym branchu)

| Etap | Status | Co |
|---|---|---|
| 0 | wdrożony | ten katalog: bootstrap (stan, budżety), szkielet `envs/prod`, ADR-y |
| 1 | wdrożony | moduły `dns` (opcjonalny) i `frontend` (S3 + CloudFront), `tools/deploy/frontend.mjs`, zadania VS Code |
| 2 | wdrożony | game-server: wiele pokoi, bilety (`crates/ticket`), `/health`, sprawdzanie originu; moduł `game_server` (EC2), `tools/deploy/game-server.mjs`. Serwer działa w trybie otwartym (bez klucza biletów) |
| 2b | do zrobienia | symulacja-cień na serwerze (autorytatywny hash), hashe mapy w `Welcome` (docs/adr/0006) |
| 3 | do zrobienia | crate `crates/meta` (Lambda w Ruście) – lobby; moduł `meta` (HTTP API, DynamoDB, klucze biletów w SSM); heartbeat pokoi z game-servera do DynamoDB |
| 4 | do zrobienia | ekran lobby i adres WebSocketu z biletem we frontendzie – razem z etapem 3 (parametr SSM z kluczem biletów przełącza serwer na bilety) |
| 5 | w części | CI (GitHub Actions: fmt, clippy, testy, determinizm, build frontendu, Terraform) – gotowe; do zrobienia: alarmy CloudWatch (błędy Lambdy, 5xx), zadanie „deploy all” |

Każdy etap na osobnym branchu `claude/aws-<etap>`, łączony do `main` fast-forwardem.

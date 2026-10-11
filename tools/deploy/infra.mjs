// Infrastruktura: build Lambdy lobby (Linux arm64), potem `terraform plan` albo `apply`.
// Terraform pakuje binarkę `bootstrap` do zipa (archive_file) i wdraża ją przy zmianie hasha –
// dlatego binarka musi istnieć przed planem.
//
//   node tools/deploy/infra.mjs           # build + terraform apply (pyta o potwierdzenie)
//   node tools/deploy/infra.mjs --plan    # build + terraform plan
//   node tools/deploy/infra.mjs --skip-build
//   node tools/deploy/infra.mjs --yes     # apply bez pytania (deploy all po przejrzeniu planu)
import { ENV_DIR, run, zigbuild } from './aws.mjs';

const args = new Set(process.argv.slice(2));

if (!args.has('--skip-build')) zigbuild(['-p', 'game-meta']);
if (args.has('--plan')) run('terraform', [`-chdir=${ENV_DIR}`, 'plan']);
else run('terraform', [`-chdir=${ENV_DIR}`, 'apply', ...(args.has('--yes') ? ['-auto-approve'] : [])]);

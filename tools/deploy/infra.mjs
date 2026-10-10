// Infrastruktura: build Lambdy lobby (Linux arm64), potem `terraform plan` albo `apply`.
// Terraform pakuje binarkę `bootstrap` do zipa (archive_file) i wdraża ją przy zmianie hasha –
// dlatego binarka musi istnieć przed planem.
//
//   node tools/deploy/infra.mjs           # build + terraform apply (pyta o potwierdzenie)
//   node tools/deploy/infra.mjs --plan    # build + terraform plan
//   node tools/deploy/infra.mjs --skip-build
import { ENV_DIR, ROOT, run } from './aws.mjs';

const TARGET = 'aarch64-unknown-linux-musl';
const args = new Set(process.argv.slice(2));

if (!args.has('--skip-build')) {
  try {
    run('cargo', ['zigbuild', '--release', '-p', 'game-meta', '--target', TARGET], { cwd: ROOT });
  } catch (e) {
    console.error(`\nBuild nie wyszedł. Potrzebne: rustup target add ${TARGET}; winget install zig.zig; cargo install cargo-zigbuild`);
    throw e;
  }
}
run('terraform', [`-chdir=${ENV_DIR}`, args.has('--plan') ? 'plan' : 'apply']);

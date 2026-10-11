#!/usr/bin/env python3
"""Build self-contained CLI distributions; Python is needed only by maintainers."""
import argparse, hashlib, json, os, re, shutil, subprocess, tarfile, tempfile, tomllib, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target')
    parser.add_argument('--no-build', action='store_true')
    parser.add_argument('--binaries', type=Path, help='Existing binaries for --no-build')
    parser.add_argument('--web', type=Path, help='Existing client build for --no-build')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/cli')
    parser.add_argument('--no-embedded-db', action='store_true', help='Development/test bundles only; require external PostgreSQL')
    args = parser.parse_args()
    host = subprocess.check_output(['rustc', '-vV'], text=True).split('host: ')[1].splitlines()[0]
    target = args.target or host
    if not args.no_build and target != host: parser.error('build CLI bundles on a native '+target+' host so embedded PostgreSQL matches the binaries')
    supported = {'x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc'}
    if target not in supported: parser.error('unsupported distribution target: ' + target)
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    if not args.no_build:
        subprocess.run(['npm', 'run', 'build:client'], cwd=ROOT / 'frontend', check=True, shell=os.name == 'nt')
        subprocess.run(['cargo', 'build', '--locked', '--release', '--target', target, '-p', 'lingshu-cli', '-p', 'lingshu-mcp'], cwd=ROOT, check=True)
        command = ['cargo', 'build', '--locked', '--release', '--target', target, '-p', 'lingshu-server']
        if not args.no_embedded_db: command += ['--features', 'desktop-embedded-db']
        subprocess.run(command, cwd=ROOT, check=True)
    binaries = (args.binaries or ROOT / 'target' / target / 'release').resolve()
    web = (args.web or ROOT / 'frontend/dist').resolve()
    if not (web / 'index.html').is_file(): parser.error('web build missing index.html')
    if (web / 'pet.html').exists(): parser.error('use the client build (npm run build:client), not the development build')
    args.output.mkdir(parents=True, exist_ok=True)
    windows = target.endswith('windows-msvc')
    name = 'lingshu-' + target + ('.zip' if windows else '.tar.gz')
    archive = args.output.resolve() / name
    with tempfile.TemporaryDirectory(prefix='lingshu-package-') as directory:
        stage = Path(directory)
        for binary in ('lingshu', 'lingshu-server', 'lingshu-mcp'):
            filename = binary + ('.exe' if windows else '')
            shutil.copy2(binaries / filename, stage / filename)
        shutil.copytree(web, stage / 'web')
        for filename in ('install.sh', 'install.ps1'): shutil.copy2(ROOT / 'scripts' / filename, stage / filename)
        shutil.copy2(ROOT / 'LICENSE', stage / 'LICENSE')
        (stage / 'VERSION').write_text(version + '\n')
        (stage / 'TARGET').write_text(target + '\n')
        metadata = {'application': 'lingshu-cli', 'version': version, 'target': target, 'embedded_database': not args.no_embedded_db}
        if target.endswith('linux-gnu'):
            versions = set()
            for binary in ('lingshu', 'lingshu-server', 'lingshu-mcp'):
                versions.update(tuple(map(int, match.decode().split('.'))) for match in re.findall(rb'GLIBC_([0-9]+(?:\.[0-9]+)+)', (stage / binary).read_bytes()))
            if versions:
                metadata['glibc_min'] = '.'.join(map(str, max(versions)))
                (stage / 'GLIBC_MIN').write_text(metadata['glibc_min'] + '\n')
        (stage / 'release.json').write_text(json.dumps(metadata, indent=2) + '\n')
        if windows:
            sources = [binaries, ROOT / 'src-tauri/resources', Path(os.environ.get('WINDIR', 'C:/Windows')) / 'System32']
            for filename in ('vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll'):
                source = next((directory / filename for directory in sources if (directory / filename).is_file()), None)
                if source: shutil.copy2(source, stage / filename)
                elif filename == 'vcruntime140.dll': parser.error('vcruntime140.dll missing; install Microsoft C runtime/build tools')
            with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as package:
                for file in stage.rglob('*'):
                    if file.is_file(): package.write(file, file.relative_to(stage))
        else:
            with tarfile.open(archive, 'w:gz') as package:
                for file in sorted(stage.iterdir()): package.add(file, arcname=file.name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    Path(str(archive) + '.sha256').write_text(digest + '  ' + archive.name + '\n')
    print(archive)
    print('SHA256:', digest)

if __name__ == '__main__': main()

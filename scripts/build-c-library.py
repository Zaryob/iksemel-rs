#!/usr/bin/env python3
"""Build the complete C shared library, including the variadic filter shim."""
import argparse, pathlib, shlex, subprocess, sys
root = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--release', action='store_true')
args = parser.parse_args()
command = ['cargo', 'rustc', '-p', 'iksemel-ffi', '--lib']
if args.release:
    command.append('--release')
result = subprocess.run(command + ['--', '--print', 'native-static-libs'], cwd=root,
                        text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
print(result.stdout, end='')
if result.returncode:
    sys.exit(result.returncode)
flags = next((line.split('native-static-libs:', 1)[1] for line in result.stdout.splitlines()
              if 'native-static-libs:' in line), None)
if flags is None:
    raise SystemExit('Cargo did not report the required static native libraries')
out = root / 'target' / ('release' if args.release else 'debug')
name = 'libiksemel.dylib' if sys.platform == 'darwin' else 'libiksemel.so'
link = ['cc', '-dynamiclib' if sys.platform == 'darwin' else '-shared', '-fPIC',
        str(root / 'ffi/variadic.c'), str(out / 'libiksemel_c.a')]
if sys.platform == 'darwin':
    link += ['-Wl,-install_name,@rpath/' + name]
link += shlex.split(flags) + ['-o', str(out / name)]
subprocess.run(link, check=True)
print(out / name)

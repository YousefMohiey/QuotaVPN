#!/usr/bin/env python3
"""Strip the native libraries inside an APK, editing the archive in place.

Why not rebuild the zip: two entry properties decide whether Android installs the
result, and a naive repack destroys both.

  - resources.arsc must be STORED (uncompressed) from targetSdk 30 onward, or the
    package is rejected at install time.
  - with extractNativeLibs=false the libraries are loaded straight out of the
    archive, so lib/*/*.so must be STORED too.

Editing in place keeps every other entry exactly as it was: same order, same
compress_type, same attributes. Only the stripped libraries are written fresh, and
they are written STORED.

Usage:
  apk-strip.py <apk> <llvm-strip-binary> [--verify-only]

Exit codes: 0 ok, 1 packaging is unsafe, 2 strip failed to remove symbols.
"""
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile

# Entries Android refuses to see compressed.
MUST_STORE = {"resources.arsc"}


def fail(code, message):
    print(f"apk-strip: {message}", file=sys.stderr)
    sys.exit(code)


def read_entries(path):
    with zipfile.ZipFile(path) as z:
        return {i.filename: (i.compress_type, i.date_time, i.external_attr, i.create_system)
                for i in z.infolist()}


def check_stored(path):
    """Every entry that must be stored, is. Returns a list of offenders."""
    names = {"resources.arsc"}
    with zipfile.ZipFile(path) as z:
        names |= {n for n in z.namelist() if n.startswith("lib/") and n.endswith(".so")}
        bad = [n for n in sorted(names)
               if n in z.namelist() and z.getinfo(n).compress_type != zipfile.ZIP_STORED]
    return bad


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def has_symtab(path):
    with open(path, "rb") as f:
        return b".symtab\x00" in f.read()


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    verify_only = "--verify-only" in sys.argv
    if not args:
        fail(1, "usage: apk-strip.py <apk> <llvm-strip> | apk-strip.py <apk> --verify-only")
    apk = args[0]
    stripper = args[1] if len(args) > 1 else None

    if not os.path.isfile(apk):
        fail(1, f"no such apk: {apk}")

    if verify_only:
        bad = check_stored(apk)
        if bad:
            fail(1, "these entries are compressed and must be stored: " + ", ".join(bad))
        print("apk-strip: stored requirements hold")
        return

    if not stripper:
        fail(1, "no llvm-strip given; pass it as the second argument")
    if not os.path.isfile(stripper):
        fail(1, f"llvm-strip not found at {stripper}")

    with zipfile.ZipFile(apk) as z:
        libs = [n for n in z.namelist() if n.startswith("lib/") and n.endswith(".so")]

    staged = {}
    tmp = tempfile.mkdtemp(prefix="apkstrip-")
    try:
        with zipfile.ZipFile(apk) as z:
            for name in libs:
                out = os.path.join(tmp, os.path.basename(name))
                with open(out, "wb") as f:
                    f.write(z.read(name))
                staged[name] = out
        del libs

        replacements = {}
        saved = 0
        for name, path in staged.items():
            before = os.path.getsize(path)
            hash_before = digest(path)
            try:
                res = subprocess.run([stripper, "--strip-unneeded", path],
                                     capture_output=True, text=True)
            except OSError as e:
                fail(2, f"could not run the stripper at {stripper}: {e}")
            if res.returncode != 0:
                fail(2, f"llvm-strip failed on {name}: {res.stderr.strip()}")
            after = os.path.getsize(path)
            hash_after = digest(path)
            if after >= before and has_symtab(path):
                fail(2, f"{name} still carries .symtab after stripping, so the step did nothing")
            saved += before - after
            print(f"apk-strip: {name}: {before} -> {after}")
            # Content, not size: a strip that drops .symtab without changing the
            # length still has to be written back, or the APK keeps the symbols.
            if hash_after != hash_before:
                with open(path, "rb") as f:
                    replacements[name] = f.read()

        if not replacements:
            print("apk-strip: nothing to replace, archive untouched")
            return

        before_meta = read_entries(apk)
        tmp_out = apk + ".rewriting"
        with zipfile.ZipFile(apk) as zin, zipfile.ZipFile(tmp_out, "w") as zout:
            for info in zin.infolist():
                data = replacements.get(info.filename)
                if data is None:
                    data = zin.read(info.filename)
                    ctype = info.compress_type
                else:
                    ctype = zipfile.ZIP_STORED
                new = zipfile.ZipInfo(info.filename, date_time=info.date_time)
                new.compress_type = ctype
                new.external_attr = info.external_attr
                new.create_system = info.create_system
                zout.writestr(new, data)

        # every non-replaced entry keeps its own compression method
        after_meta = read_entries(tmp_out)
        for name, (ctype, *_rest) in before_meta.items():
            if name in replacements:
                continue
            if after_meta[name][0] != ctype:
                os.unlink(tmp_out)
                fail(1, f"{name} changed compression during the rewrite")

        bad = check_stored(tmp_out)
        if bad:
            os.unlink(tmp_out)
            fail(1, "these entries are compressed and must be stored: " + ", ".join(bad))

        os.replace(tmp_out, apk)
        print(f"apk-strip: saved {saved} bytes, archive still installable")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()

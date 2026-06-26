#!/usr/bin/env python3
import os, re, subprocess, sys
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)
import split_phase3 as s3

def main():
    for job in s3.SPLIT_JOBS:
        job.parts = [p for p in job.parts]  # copy
    # fix preview/context ranges
    for job in s3.SPLIT_JOBS:
        if job.rel_src.endswith("build_node_context.rs"):
            job.parts = [
                s3.PartSpec("preview", 15, 116),
                s3.PartSpec("context", 117, 567),
                s3.PartSpec("helpers", 568, 577),
            ]
    for job in s3.SPLIT_JOBS:
        s3.apply_job(job)
    s3.post_fixes()
    s3.fix_split_module_paths(s3.KERNEL_SRC)
    s3.remove_duplicate_modules()
    # preview trim orphan doc + path fix
    prev = os.path.join(s3.KERNEL_SRC, "compile/build_node_context/preview.rs")
    if os.path.isfile(prev):
        t = open(prev).read()
        idx = t.find("/// Resolved preview")
        if idx > 0:
            t = t[:idx]
        t = t.replace("super::build_experience::", "crate::compile::build_experience::")
        open(prev, "w").write(t)
    s3.report_counts()
    for _ in range(3):
        r = subprocess.run(
            ["cargo", "build", "-p", "mei-lang-server"],
            cwd=os.path.join(SCRIPT_DIR, "..", "..", ".."),
            capture_output=True,
            text=True,
        )
        out = r.stdout + r.stderr
        if r.returncode == 0:
            print(out.split("Finished")[-1] if "Finished" in out else "BUILD OK")
            w = len(re.findall(r"^warning:", out, re.M))
            print(f"warnings: {w}")
            return 0
        print(out[-3000:])
        if "preview.rs" in out and "unclosed" in out:
            open(prev, "a").write("\n}\n")
            continue
        return r.returncode
    return 1

if __name__ == "__main__":
    sys.exit(main())

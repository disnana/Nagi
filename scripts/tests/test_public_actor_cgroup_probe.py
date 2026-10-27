import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location(
    "public_actor_cgroup_probe", ROOT / "scripts" / "public_actor_cgroup_probe.py")
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)
IMAGE_SPEC = importlib.util.spec_from_file_location(
    "prepare_public_actor_cgroup_image",
    ROOT / "scripts" / "prepare_public_actor_cgroup_image.py")
IMAGE = importlib.util.module_from_spec(IMAGE_SPEC)
IMAGE_SPEC.loader.exec_module(IMAGE)


class PublicActorCgroupProbeTests(unittest.TestCase):
    def test_ldd_parser_keeps_absolute_dependencies_and_rejects_missing(self):
        output = """\
linux-vdso.so.1 (0x0001)
libm.so.6 => /lib/libm.so.6 (0x0002)
/lib64/ld-linux-x86-64.so.2 (0x0003)
"""
        self.assertEqual(IMAGE.parse_ldd(output), [
            Path("/lib/libm.so.6"), Path("/lib64/ld-linux-x86-64.so.2")])
        with self.assertRaises(RuntimeError):
            IMAGE.parse_ldd("libmissing.so => not found\n")
        with self.assertRaises(RuntimeError):
            IMAGE.parse_ldd("/../../tmp/liboutside.so (0x0004)\n")

    def test_dependency_copy_follows_normal_symlink_and_rejects_collision(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source_root = base / "source"
            source_root.mkdir()
            library = source_root / "libactual.so"
            library.write_bytes(b"trusted library")
            link = source_root / "libalias.so"
            link.symlink_to(library.name)
            image_root = base / "image"
            target, copied_hash = IMAGE.copy_dependency(image_root, link)
            self.assertEqual(target.read_bytes(), b"trusted library")
            self.assertFalse(target.is_symlink())
            self.assertEqual(copied_hash, IMAGE.sha256(target))
            with self.assertRaises(RuntimeError):
                IMAGE.copy_dependency(image_root, link)

    def test_nested_namespace_parent_maps_under_host_cgroup_root(self):
        root = Path("/test-cgroup")
        self.assertEqual(
            PROBE.normalized_cgroup_directory(
                "/../docker/0123456789abcdef", root),
            root / "docker" / "0123456789abcdef")

    def test_cgroup_path_rejects_internal_or_empty_traversal(self):
        for value in ("/", "/docker/../other", "/docker//child", "/docker/./child"):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                PROBE.normalized_cgroup_directory(value)

    def test_host_config_requires_hard_limits_and_isolation(self):
        expected = {
            "Memory": 1024**3,
            "MemorySwap": 1024**3,
            "NanoCpus": 2_000_000_000,
            "CpusetCpus": "0,1",
            "PidsLimit": 512,
            "ReadonlyRootfs": True,
            "NetworkMode": "none",
            "CapDrop": ["ALL"],
            "SecurityOpt": ["no-new-privileges"],
        }
        PROBE.validate_host_config(expected, 2, "0,1")
        for key, wrong in (("Memory", 0), ("NanoCpus", 1_000_000_000),
                           ("NetworkMode", "bridge"), ("CapDrop", [])):
            changed = dict(expected)
            changed[key] = wrong
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                PROBE.validate_host_config(changed, 2, "0,1")

    def test_snapshot_keeps_oom_and_cpu_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "cpu.stat").write_text("usage_usec 30\nuser_usec 20\nsystem_usec 10\n")
            (root / "memory.events").write_text(
                "low 0\nhigh 0\nmax 1\noom 0\noom_kill 0\noom_group_kill 0\n")
            (root / "memory.current").write_text("1024\n")
            (root / "memory.peak").write_text("2048\n")
            (root / "pids.current").write_text("2\n")
            (root / "memory.max").write_text(f"{1024**3}\n")
            (root / "memory.swap.max").write_text("0\n")
            (root / "cpu.max").write_text("200000 100000\n")
            (root / "cpuset.cpus.effective").write_text("0-1\n")
            (root / "pids.max").write_text("512\n")
            snapshot = PROBE.cgroup_snapshot(root)
            self.assertEqual(snapshot, {
                "memory_current_bytes": 1024,
                "memory_peak_bytes": 2048,
                "pids_current": 2,
                "cpu_usage_usec": 30,
                "cpu_user_usec": 20,
                "cpu_system_usec": 10,
                "memory_events": {
                    "low": 0, "high": 0, "max": 1, "oom": 0,
                    "oom_kill": 0, "oom_group_kill": 0,
                },
                "memory_max": str(1024**3),
                "memory_swap_max": "0",
                "cpu_max": "200000 100000",
                "cpuset_cpus_effective": "0-1",
                "pids_max": "512",
            })
            PROBE.validate_cgroup_limits(snapshot, 2, [0, 1])
            snapshot["memory_max"] = "max"
            with self.assertRaises(RuntimeError):
                PROBE.validate_cgroup_limits(snapshot, 2, [0, 1])

    def test_cpu_set_parser_rejects_reversed_and_duplicate_ranges(self):
        self.assertEqual(PROBE.expand_cpu_set("0-2,4"), [0, 1, 2, 4])
        for value in ("2-1", "0,0", "0-1,1"):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                PROBE.expand_cpu_set(value)

    def test_failed_container_cleanup_changes_passed_case_to_failure(self):
        result = {"status": "passed"}
        failed = mock.Mock(returncode=1, stderr="daemon refused cleanup", stdout="")
        with mock.patch.object(PROBE.subprocess, "run", return_value=failed):
            PROBE.cleanup_container(result, "owned-container")
        self.assertEqual(result["status"], "failed")
        self.assertIn("docker rm failed (1)", result["error"])

    def test_residual_container_changes_passed_case_to_failure(self):
        result = {"status": "passed"}
        removed = mock.Mock(returncode=0, stderr="", stdout="owned-container\n")
        residual = mock.Mock(returncode=0, stderr="", stdout="owned-container\n")
        with mock.patch.object(PROBE.subprocess, "run", side_effect=[removed, residual]):
            PROBE.cleanup_container(result, "owned-container")
        self.assertEqual(result["status"], "failed")
        self.assertIn("remains after cleanup", result["error"])

    def test_successful_cleanup_records_empty_residual_set(self):
        result = {"status": "passed"}
        removed = mock.Mock(returncode=0, stderr="", stdout="owned-container\n")
        absent = mock.Mock(returncode=0, stderr="", stdout="")
        with mock.patch.object(PROBE.subprocess, "run", side_effect=[removed, absent]):
            PROBE.cleanup_container(result, "owned-container")
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["cleanup"], {
            "rm_returncode": 0, "residual_container_ids": []})


if __name__ == "__main__":
    unittest.main()

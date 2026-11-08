"""Tests of `shards.py`, run from the repository's root with
`python3 -m unittest discover -s python-parity -p test_shards.py`."""

import unittest

import shards


class DealTest(unittest.TestCase):
    def test_every_test_runs_on_exactly_one_shard(self):
        times = {f"slow_{index}": float(index) for index in range(40)}
        tests = sorted(times) + ["new_one", "new_two"]
        dealt = shards.deal(tests, times, 5)
        everything = [name for shard in dealt for name in shard]
        self.assertEqual(sorted(everything), sorted(tests))

    def test_known_tests_are_dealt_slowest_first_to_the_lightest_shard(self):
        times = {"a": 10.0, "b": 9.0, "c": 6.0, "d": 4.0, "e": 1.0}
        # a, then b, c on b, d and e on a: fifteen seconds each.
        self.assertEqual(shards.deal(sorted(times), times, 2), [["a", "d", "e"], ["b", "c"]])

    def test_a_new_test_moves_no_other(self):
        times = {f"t{index}": float(index % 7 + 1) for index in range(30)}
        before = shards.deal(sorted(times), times, 5)
        after = shards.deal(sorted(times) + ["brand_new"], times, 5)
        placed = shards.hashed_shard("brand_new", 5)
        for index, (old, new) in enumerate(zip(before, after)):
            expected = sorted(old + ["brand_new"]) if index == placed else old
            self.assertEqual(new, expected)

    def test_a_test_no_longer_listed_is_left_out(self):
        times = {"kept": 1.0, "gone": 100.0}
        self.assertEqual(shards.deal(["kept"], times, 2), [["kept"], []])

    def test_the_dealing_is_the_same_every_run(self):
        self.assertEqual(shards.hashed_shard("some_parity::a_test", 5), 1)


class OutputTest(unittest.TestCase):
    def test_a_filterset_names_each_test_exactly(self):
        self.assertEqual(shards.filterset(["a::b", "c"]), "test(=a::b) | test(=c)")
        self.assertEqual(shards.filterset([]), "none()")

    def test_the_slowest_start_first(self):
        config = shards.priorities(["quick", "slow", "unknown"], {"quick": 1.0, "slow": 9.0})
        self.assertEqual(
            config,
            "[[profile.default.overrides]]\nfilter = 'test(=slow)'\npriority = 100\n\n"
            "[[profile.default.overrides]]\nfilter = 'test(=quick)'\npriority = 99\n",
        )

    def test_wall_time_runs_the_slowest_first_on_each_thread(self):
        self.assertEqual(shards.wall_time([5.0, 3.0, 3.0, 2.0], 2), 7.0)
        self.assertEqual(shards.wall_time([], 4), 0.0)

    def test_times_are_read_from_nextest_output(self):
        log = (
            "2026-11-07T23:32:28.1124321Z \x1b[32;1m        PASS\x1b[0m [   3.007s] ( 3/57) "
            "\x1b[35;1mmusic21-rs-python-parity::parity\x1b[0m \x1b[36mcapella_parity\x1b[0m"
            "\x1b[36m::\x1b[0m\x1b[34;1mthe_crate_reads_capella\x1b[0m\n"
            "        FAIL [   1.000s] ( 4/57) music21-rs-python-parity::parity broken::test\n"
        )
        self.assertEqual(
            shards.times_from_logs([log]), {"capella_parity::the_crate_reads_capella": 3.007}
        )
        self.assertEqual(
            shards.times_toml({"b": 2.04, "a": 1.0}).splitlines()[-2:],
            ['"a" = 1.0', '"b" = 2.0'],
        )

    def test_the_listing_gives_every_binary_s_tests(self):
        listing = {
            "rust-suites": {
                "lib": {"testcases": {"doctest::tests::x": {}}},
                "lib::parity": {"testcases": {"abc::y": {}, "abc::z": {}}},
            }
        }
        self.assertEqual(shards.listed_tests(listing), ["abc::y", "abc::z", "doctest::tests::x"])

    def test_the_checked_in_times_are_read(self):
        times = shards.load_times()
        self.assertGreater(len(times), 200)
        self.assertTrue(all(seconds >= 0 for seconds in times.values()))


if __name__ == "__main__":
    unittest.main()

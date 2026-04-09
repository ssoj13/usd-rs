#!/usr/bin/env python3
"""usd-rs: pxr.Work thread-limit bindings (usd_work / PyO3)."""

import unittest

from pxr import Work


class TestWorkBindings(unittest.TestCase):
    def test_functions_exist(self):
        self.assertTrue(callable(Work.SetMaximumConcurrencyLimit))
        self.assertTrue(callable(Work.SetConcurrencyLimit))
        self.assertTrue(callable(Work.GetConcurrencyLimit))
        self.assertTrue(callable(Work.GetPhysicalConcurrencyLimit))

    def test_get_limits_sane(self):
        phys = Work.GetPhysicalConcurrencyLimit()
        self.assertIsInstance(phys, int)
        self.assertGreaterEqual(phys, 1)

        cur = Work.GetConcurrencyLimit()
        self.assertIsInstance(cur, int)
        self.assertGreaterEqual(cur, 1)

    def test_set_maximum_then_readable(self):
        Work.SetMaximumConcurrencyLimit()
        cur = Work.GetConcurrencyLimit()
        self.assertGreaterEqual(cur, 1)

    def test_set_concurrency_roundtrip(self):
        phys = Work.GetPhysicalConcurrencyLimit()
        target = max(1, min(2, phys))
        Work.SetConcurrencyLimit(target)
        self.assertEqual(Work.GetConcurrencyLimit(), target)
        Work.SetMaximumConcurrencyLimit()


if __name__ == "__main__":
    unittest.main(verbosity=2)

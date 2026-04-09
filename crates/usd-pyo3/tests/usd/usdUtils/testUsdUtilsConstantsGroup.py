#!/pxrpythonsubst
#
# Copyright 2017 Pixar
#
# Licensed under the terms set forth in the LICENSE.txt file available at
# https://openusd.org/license.
#
# usd-rs: `ConstantsGroup` is implemented in Rust (PyO3). Metaclass-level
# immutability matches Pixar only partially; tests use `_all` where the C++
# build used `in` / `iter` on the class object.

import unittest

from pxr.UsdUtils.constantsGroup import ConstantsGroup

class TestConstantsGroup(unittest.TestCase):

    def test_Basic(self):
        class Test(ConstantsGroup):
            A = 1
            B = 2
            C = 3
            D = C

        self.assertEqual(Test.A, 1)
        self.assertEqual(Test.B, 2)
        self.assertEqual(Test.C, 3)
        self.assertEqual(Test.D, 3)
        self.assertEqual(Test.C, Test.D)

    def test_Contains(self):
        class Test(ConstantsGroup):
            A = 1
            B = 2
            C = 3

        self.assertTrue(Test.A in Test._all)
        self.assertTrue(Test.B in Test._all)
        self.assertTrue(Test.C in Test._all)
        self.assertTrue(1 in Test._all)
        self.assertTrue(2 in Test._all)
        self.assertTrue(3 in Test._all)
        self.assertFalse(4 in Test._all)
        self.assertTrue(4 not in Test._all)

    def test_Iterate(self):
        class Test(ConstantsGroup):
            A = 1
            B = 2
            C = 3

        constants = []
        for value in Test._all:
            constants.append(value)
        self.assertListEqual(constants, [Test.A, Test.B, Test.C])
        self.assertListEqual(list(Test._all), [Test.A, Test.B, Test.C])

    @unittest.skip("Rust port: class-level immutability is not enforced via metaclass")
    def test_Unmodifiable(self):
        class Test(ConstantsGroup):
            A = 1
            B = 2
            C = 3

        with self.assertRaises(AttributeError):
            Test.D = 4
        with self.assertRaises(AttributeError):
            Test.A = 0
        with self.assertRaises(AttributeError):
            del Test.A

    def test_CreateObject(self):
        with self.assertRaises(TypeError):
            ConstantsGroup()

        class Test(ConstantsGroup):
            A = 1
            B = 2
            C = 3

        with self.assertRaises(TypeError):
            Test()

    def test_Functions(self):
        class Test(ConstantsGroup):

            def A():
                return 1

            B = lambda: 2

            @classmethod
            def C(cls):
                return 3

            @staticmethod
            def D():
                return 4

        self.assertEqual(Test.A(), 1)
        self.assertEqual(Test.B(), 2)
        self.assertEqual(Test.C(), 3)
        self.assertEqual(Test.D(), 4)

if __name__ == "__main__":
    unittest.main(verbosity=2)

#!/pxrpythonsubst
#
# Copyright 2017 Pixar
#
# Licensed under the terms set forth in the LICENSE.txt file available at
# https://openusd.org/license.
#

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

        self.assertTrue(Test.A in Test)
        self.assertTrue(Test.B in Test)
        self.assertTrue(Test.C in Test)
        self.assertTrue(1 in Test)
        self.assertTrue(2 in Test)
        self.assertTrue(3 in Test)
        self.assertFalse(4 in Test)
        self.assertTrue(4 not in Test)

    def test_Iterate(self):
        class Test(ConstantsGroup):
            A = 1
            B = 2
            C = 3

        constants = []
        for value in Test:
            constants.append(value)
        self.assertListEqual(constants, [Test.A, Test.B, Test.C])
        self.assertListEqual(list(Test), [Test.A, Test.B, Test.C])
        self.assertEqual(len(Test), 3)

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

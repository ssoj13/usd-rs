#
# Copyright 2016 Pixar
#
# Licensed under the terms set forth in the LICENSE.txt file available at
# https://openusd.org/license.
#
from pxr_rs import Plug, Tf

class TestPlugPythonLoaded(Plug._TestPlugBase1):
    def GetTypeName(self):
        return 'TestPlugPythonLoaded'
Tf.Type.Define(TestPlugPythonLoaded)

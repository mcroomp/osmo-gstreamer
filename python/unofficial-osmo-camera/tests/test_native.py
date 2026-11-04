import unittest

import osmo_camera


class NativeBindingTests(unittest.TestCase):
    def test_native_version(self):
        self.assertEqual(osmo_camera.version(), "0.2.0")


if __name__ == "__main__":
    unittest.main()

import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.build_android_player import verify_game_classification


class GameClassificationTests(unittest.TestCase):
    @patch("tools.build_android_player.subprocess.run")
    def test_inspects_the_packaged_apk(self, run):
        run.return_value = subprocess.CompletedProcess(
            [], 0, stdout="application-label:'LambdaDX Player'\napplication-isGame\n",
        )
        env = {"ANDROID_HOME": "/sdk"}
        apk = Path("final-player.apk")
        verify_game_classification(apk, env)
        run.assert_called_once_with(
            ["/sdk/build-tools/36.1.0/aapt", "dump", "badging", str(apk)],
            env=env, check=True, capture_output=True, text=True,
        )

    @patch("tools.build_android_player.subprocess.run")
    def test_rejects_missing_game_marker(self, run):
        for output in ["", "application-label:'application-isGame'\n"]:
            with self.subTest(output=output):
                run.return_value = subprocess.CompletedProcess([], 0, stdout=output)
                with self.assertRaisesRegex(RuntimeError, "not marked as an Android game"):
                    verify_game_classification(Path("player.apk"), {"ANDROID_HOME": "/sdk"})

    @patch("tools.build_android_player.subprocess.run")
    def test_failed_apk_inspection_is_not_accepted(self, run):
        run.side_effect = subprocess.CalledProcessError(1, "aapt")
        with self.assertRaises(subprocess.CalledProcessError):
            verify_game_classification(Path("player.apk"), {"ANDROID_HOME": "/sdk"})


if __name__ == "__main__":
    unittest.main()

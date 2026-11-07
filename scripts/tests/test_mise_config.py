from pathlib import Path
import tomllib
import unittest


CONFIG = Path(__file__).resolve().parents[2] / "mise.toml"


class MiseConfigTests(unittest.TestCase):
    def test_tools_and_tasks_are_alphabetically_ordered(self):
        config = tomllib.loads(CONFIG.read_text())
        for section in ["tools", "tasks"]:
            with self.subTest(section=section):
                keys = list(config[section])
                self.assertEqual(keys, sorted(keys))


if __name__ == "__main__":
    unittest.main()

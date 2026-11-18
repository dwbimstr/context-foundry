import importlib.util
import itertools
import math
import json
import subprocess
import sys
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('pilot_v2_power', Path(__file__).with_name('pilot-v2-power.py'))
power = importlib.util.module_from_spec(spec)
spec.loader.exec_module(power)


def example():
    return {'overall_alpha': 0.05, 'confirmatory_contrasts': 3, 'noninferiority_margin': 0.1,
            'required_discovery_gain': 0.0, 'task_counts': [10, 50], 'repetitions_per_task': 2,
            'scenarios': [{'name': 'synthetic', 'success': {'baseline': 0.7, 'gain': 0.2, 'loss': 0.1},
                           'discovery': {'baseline': 0.4, 'gain': 0.3, 'loss': 0.1}}]}


class PowerTests(unittest.TestCase):
    def test_distribution_matches_independent_exhaustive_enumeration(self):
        gain, loss, n = 0.3, 0.2, 4
        expected = [0.0] * (2 * n + 1)
        for outcomes in itertools.product((-1, 0, 1), repeat=n):
            p = math.prod({-1: loss, 0: 1 - gain - loss, 1: gain}[v] for v in outcomes)
            expected[sum(outcomes) + n] += p
        for actual, wanted in zip(power.difference_distribution(n, gain, loss), expected):
            self.assertAlmostEqual(actual, wanted, places=14)

    def test_extreme_pairs_and_cluster_repetition(self):
        a = example()
        a['task_counts'] = [100]
        a['scenarios'][0]['success'] = {'baseline': 0.0, 'gain': 1.0, 'loss': 0.0}
        a['scenarios'][0]['discovery'] = {'baseline': 0.0, 'gain': 1.0, 'loss': 0.0}
        first = power.calculate(a)
        self.assertEqual(first['results'][0]['joint_power_lower_bound'], 1.0)
        a['repetitions_per_task'] = 99
        self.assertEqual(first['results'], power.calculate(a)['results'])
        self.assertFalse(first['execution_authorized'])

    def test_invalid_probability_and_limits_are_refused(self):
        for change in ('baseline', 'nan', 'huge', 'duplicate_count', 'unknown'):
            data = example()
            if change == 'baseline': data['scenarios'][0]['success']['gain'] = 0.9
            if change == 'nan': data['overall_alpha'] = float('nan')
            if change == 'huge': data['overall_alpha'] = 10**400
            if change == 'duplicate_count': data['task_counts'] = [10, 10]
            if change == 'unknown': data['extra'] = True
            with self.subTest(change=change), self.assertRaises(ValueError): power.calculate(data)

    def test_cli_rejects_duplicate_keys_and_writes_no_result(self):
        path = Path(__file__).with_name('pilot-v2-power.py')
        bad = subprocess.run([sys.executable, str(path)], input='{"overall_alpha":0.05,"overall_alpha":0.1}',
                             text=True, capture_output=True, check=False)
        self.assertEqual(bad.returncode, 2)
        self.assertEqual(bad.stdout, '')
        self.assertIn('duplicate key', bad.stderr)
        good = subprocess.run([sys.executable, str(path)], input=json.dumps(example()),
                              text=True, capture_output=True, check=False)
        self.assertEqual(good.returncode, 0, good.stderr)
        self.assertEqual(json.loads(good.stdout)['status'], 'sensitivity_only_not_frozen')

    def test_probability_boundary_accepts_exact_decimal_complements(self):
        data = example()
        data['scenarios'][0]['success'] = {'baseline': 0.9, 'gain': 0.1, 'loss': 0.9}
        result = power.calculate(data)
        self.assertTrue(all(0 <= row['success_power'] <= 1 for row in result['results']))

    def test_tiny_alpha_cannot_underflow_silently(self):
        data = example()
        data['overall_alpha'] = 5e-324
        with self.assertRaisesRegex(ValueError, 'underflows'):
            power.calculate(data)

    def test_noninferiority_boundary_respects_its_one_sided_tail(self):
        for n in (25, 75, 200):
            tail = 0.01
            width = math.sqrt(-2 * math.log(tail) / n)
            endpoint = {'gain': 0.10, 'loss': 0.20}
            self.assertLessEqual(power.endpoint_power(n, endpoint, -0.10, width), tail + 1e-12)

    def test_null_false_positive_probability_is_bounded(self):
        # Identical treatment/control success rates at zero NI margin, with
        # correlated pairs: the one-sided confidence test cannot exceed its tail.
        for n in (5, 20, 40):
            for discordance in (0.05, 0.25, 0.5):
                tail = 0.025
                width = math.sqrt(2 * math.log(1 / tail) / n)
                endpoint = {'gain': discordance / 2, 'loss': discordance / 2}
                self.assertLessEqual(power.endpoint_power(n, endpoint, 0.0, width), tail + 1e-12)


if __name__ == '__main__':
    unittest.main()

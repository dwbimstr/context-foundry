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

    def test_stratified_distribution_matches_exhaustive_tasks(self):
        strata = [(2, 0.3, 0.2), (2, 0.1, 0.6)]
        expected = [0.0] * 9
        rates = [(0.3, 0.2)] * 2 + [(0.1, 0.6)] * 2
        for outcomes in itertools.product((-1, 0, 1), repeat=4):
            p = math.prod({-1: loss, 0: 1 - gain - loss, 1: gain}[value]
                          for value, (gain, loss) in zip(outcomes, rates))
            expected[sum(outcomes) + 4] += p
        for actual, wanted in zip(power.stratified_distribution(strata), expected):
            self.assertAlmostEqual(actual, wanted, places=14)

    def test_identical_strata_equal_original_model_and_ignore_repetition_count(self):
        original = example()
        original['task_counts'] = [50]
        scenario = original['scenarios'][0]
        stratified = {key: original[key] for key in ('overall_alpha', 'confirmatory_contrasts',
                                                     'noninferiority_margin', 'required_discovery_gain')}
        stratified['designs'] = [{'name': 'split', 'strata': [
            {'name': 'a', 'tasks': 20, 'repetitions_per_task': 1,
             'success': scenario['success'], 'discovery': scenario['discovery']},
            {'name': 'b', 'tasks': 30, 'repetitions_per_task': 9,
             'success': scenario['success'], 'discovery': scenario['discovery']}]}]
        a = power.calculate(original)['results'][0]
        b = power.calculate(stratified)['results'][0]
        for key in ('independent_tasks', 'success_power', 'discovery_power',
                    'joint_power_lower_bound', 'one_sided_half_width'):
            self.assertEqual(a[key], b[key])
        self.assertEqual([v['tasks'] for v in b['strata']], [20, 30])
        for row in b['strata']:
            width = math.sqrt(-2 * math.log(power.calculate(stratified)['per_endpoint_tail_alpha']) / row['tasks'])
            self.assertEqual(row['success_power'], power.endpoint_power(row['tasks'], scenario['success'], -0.1, width))
        changed = power.calculate(stratified)['results'][0]
        stratified['designs'][0]['strata'][1]['repetitions_per_task'] = 100
        repeated = power.calculate(stratified)['results'][0]
        self.assertEqual(changed['success_power'], repeated['success_power'])
        self.assertEqual(changed['discovery_power'], repeated['discovery_power'])
        self.assertEqual(changed['strata'][1]['success_power'], repeated['strata'][1]['success_power'])

    def test_stratified_cli_uses_total_independent_tasks(self):
        endpoint = {'baseline': 0.5, 'gain': 0.2, 'loss': 0.1}
        data = {'overall_alpha': 0.05, 'confirmatory_contrasts': 3, 'noninferiority_margin': 0.1,
                'required_discovery_gain': 0.0, 'designs': [{'name': 'synthetic', 'strata': [
                    {'name': 'a', 'tasks': 10, 'repetitions_per_task': 1,
                     'success': endpoint, 'discovery': endpoint},
                    {'name': 'b', 'tasks': 15, 'repetitions_per_task': 5,
                     'success': endpoint, 'discovery': endpoint}]}]}
        path = Path(__file__).with_name('pilot-v2-power.py')
        result = subprocess.run([sys.executable, str(path)], input=json.dumps(data),
                                text=True, capture_output=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        output = json.loads(result.stdout)
        self.assertEqual(output['results'][0]['independent_tasks'], 25)
        self.assertEqual([x['tasks'] for x in output['results'][0]['strata']], [10, 15])
        self.assertEqual(output['per_endpoint_tail_alpha'], 0.05 / 6)
        self.assertFalse(output['execution_authorized'])

    def test_heterogeneous_strata_null_boundary_and_invalid_design(self):
        # Equal-size strata with differences +0.1 and -0.3 average to -0.1.
        n, tail = 40, 0.025
        width = math.sqrt(-2 * math.log(tail) / n)
        distribution = power.stratified_distribution([(20, 0.2, 0.1), (20, 0.1, 0.4)])
        rejection = math.fsum(p for i, p in enumerate(distribution) if (i - n) / n - width > -0.1)
        self.assertLessEqual(rejection, tail + 1e-12)
        data = {'overall_alpha': 0.05, 'confirmatory_contrasts': 1, 'noninferiority_margin': 0.1,
                'required_discovery_gain': 0.0, 'designs': [{'name': 'x', 'strata': []}]}
        with self.assertRaisesRegex(ValueError, 'strata'):
            power.calculate(data)
        endpoint = {'baseline': 0.5, 'gain': 0.1, 'loss': 0.1}
        item = {'name': 'duplicate', 'tasks': 1, 'repetitions_per_task': 1,
                'success': endpoint, 'discovery': endpoint}
        data['designs'][0]['strata'] = [item, dict(item)]
        with self.assertRaisesRegex(ValueError, 'stratum names'):
            power.calculate(data)
        data['designs'][0]['strata'][1]['name'] = 'other'
        data['designs'][0]['strata'][1]['tasks'] = 1000
        with self.assertRaisesRegex(ValueError, 'at most 1000'):
            power.calculate(data)

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

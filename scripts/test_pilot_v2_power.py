import importlib.util
import itertools
import math
import json
import subprocess
import sys
from fractions import Fraction
from pathlib import Path
from statistics import NormalDist
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

    def test_explicit_block_distributions_match_exhaustive_fractional_means(self):
        a = {'name': 'a', 'tasks': 2, 'repetitions_per_task': 2,
             'success': {'outcomes': [{'difference_sum': -2, 'probability': 0.1},
                                      {'difference_sum': 0, 'probability': 0.6},
                                      {'difference_sum': 2, 'probability': 0.3}]}}
        b = {'name': 'b', 'tasks': 1, 'repetitions_per_task': 3,
             'success': {'outcomes': [{'difference_sum': -1, 'probability': 0.25},
                                      {'difference_sum': 2, 'probability': 0.75}]}}
        scale = power._block_scale([a, b], 'success')
        self.assertEqual(scale, 6)
        actual = power.block_distribution([a, b], 'success', scale)
        expected = {}
        for x, y, z in itertools.product(a['success']['outcomes'], a['success']['outcomes'], b['success']['outcomes']):
            key = 3 * (x['difference_sum'] + y['difference_sum']) + 2 * z['difference_sum']
            expected[key] = expected.get(key, 0.0) + x['probability'] * y['probability'] * z['probability']
        self.assertEqual(set(actual), set(expected))
        for key in expected:
            self.assertAlmostEqual(actual[key], expected[key], places=14)

    def test_heterogeneous_explicit_blocks_respect_null_boundary(self):
        # Equal-size task strata have expected differences +0.1 and -0.3.
        def item(label, values):
            return {'name': label, 'tasks': 20, 'repetitions_per_task': 2,
                    'success': {'outcomes': [{'difference_sum': d, 'probability': p} for d, p in values]}}
        strata = [item('positive', [(-2, 0.1), (0, 0.7), (2, 0.2)]),
                  item('negative', [(-2, 0.4), (0, 0.5), (2, 0.1)])]
        tail = 0.025
        width = math.sqrt(-2 * math.log(tail) / 40)
        self.assertLessEqual(power.block_endpoint_power(strata, 'success', -0.1, width), tail + 1e-12)

    def test_explicit_perfect_correlation_matches_legacy(self):
        legacy = {'baseline': 0.5, 'gain': 0.2, 'loss': 0.1}
        explicit = {'outcomes': [{'difference_sum': -2, 'probability': 0.1},
                                 {'difference_sum': 0, 'probability': 0.7},
                                 {'difference_sum': 2, 'probability': 0.2}]}
        common = {'overall_alpha': 0.05, 'confirmatory_contrasts': 2, 'noninferiority_margin': 0.1,
                  'required_discovery_gain': 0.0}
        def design(endpoint):
            return {**common, 'designs': [{'name': 'x', 'strata': [
                {'name': 'a', 'tasks': 100, 'repetitions_per_task': 2,
                 'success': endpoint, 'discovery': endpoint}]}]}
        old = power.calculate(design(legacy))['results'][0]
        new = power.calculate(design(explicit))['results'][0]
        for key in ('success_power', 'discovery_power', 'joint_power_lower_bound'):
            self.assertAlmostEqual(old[key], new[key], places=14)
            self.assertAlmostEqual(old['strata'][0][key], new['strata'][0][key], places=14)

    def test_explicit_blocks_work_through_cli_with_mixed_strata(self):
        explicit = {'outcomes': [{'difference_sum': -2, 'probability': 0.1},
                                 {'difference_sum': -1, 'probability': 0.2},
                                 {'difference_sum': 0, 'probability': 0.4},
                                 {'difference_sum': 1, 'probability': 0.2},
                                 {'difference_sum': 2, 'probability': 0.1}]}
        legacy = {'baseline': 0.5, 'gain': 0.2, 'loss': 0.1}
        data = {'overall_alpha': 0.05, 'confirmatory_contrasts': 2, 'noninferiority_margin': 0.1,
                'required_discovery_gain': 0.0, 'designs': [{'name': 'mixed', 'strata': [
                    {'name': 'explicit', 'tasks': 10, 'repetitions_per_task': 2,
                     'success': explicit, 'discovery': legacy},
                    {'name': 'perfect', 'tasks': 10, 'repetitions_per_task': 3,
                     'success': legacy, 'discovery': explicit}]}]}
        path = Path(__file__).with_name('pilot-v2-power.py')
        run = subprocess.run([sys.executable, str(path)], input=json.dumps(data),
                             text=True, capture_output=True, check=False)
        self.assertEqual(run.returncode, 0, run.stderr)
        output = json.loads(run.stdout)
        self.assertEqual(output['results'][0]['independent_tasks'], 20)
        self.assertEqual([x['tasks'] for x in output['results'][0]['strata']], [10, 10])
        self.assertFalse(output['execution_authorized'])
        # An infeasible calculation refuses before dynamic programming.
        data['designs'][0]['strata'][0]['tasks'] = 500
        data['designs'][0]['strata'][1]['tasks'] = 500
        with self.assertRaisesRegex(ValueError, 'operation bound'):
            power.calculate(data)

    def test_explicit_blocks_fail_closed(self):
        def entry(outcomes, repetitions=2):
            return power._block_endpoint({'outcomes': outcomes}, repetitions, 'test')
        bad = [
            [{'difference_sum': 0, 'probability': 0.8}],
            [{'difference_sum': 0, 'probability': 0.5}, {'difference_sum': 0, 'probability': 0.5}],
            [{'difference_sum': 3, 'probability': 1.0}],
            [{'difference_sum': 0, 'probability': float('nan')}],
        ]
        for outcomes in bad:
            with self.subTest(outcomes=outcomes), self.assertRaises(ValueError):
                entry(outcomes)
        with self.assertRaisesRegex(ValueError, 'at most 12'):
            entry([{'difference_sum': 0, 'probability': 1.0}], 13)
        endpoint = {'outcomes': [{'difference_sum': 0, 'probability': 1.0}]}
        strata = [{'repetitions_per_task': 11, 'success': endpoint},
                  {'repetitions_per_task': 7, 'success': endpoint}]
        with self.assertRaisesRegex(ValueError, 'denominator'):
            power._block_scale(strata, 'success')

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


# The seven hypothetical scenarios of the 2026-11-20 sample proposal
# (bounded-pilot-v2.md, "Proposed sample and statistical design"):
# name, success (baseline, gain, loss), discovery (baseline, gain, loss).
PROPOSAL = [('H1', (0.50, 0.05, 0.05), (0.30, 0.15, 0.05)),
            ('H2', (0.50, 0.10, 0.10), (0.30, 0.25, 0.05)),
            ('H3', (0.50, 0.20, 0.20), (0.30, 0.35, 0.05)),
            ('H4', (0.30, 0.10, 0.05), (0.20, 0.45, 0.05)),
            ('H5', (0.70, 0.15, 0.05), (0.50, 0.30, 0.10)),
            ('H6', (0.50, 0.25, 0.05), (0.10, 0.55, 0.05)),
            ('H7', (0.50, 0.05, 0.15), (0.50, 0.10, 0.10))]


def scenarios(names=None):
    def endpoint(values):
        return {'baseline': values[0], 'gain': values[1], 'loss': values[2]}
    return [{'name': name, 'success': endpoint(s), 'discovery': endpoint(d)}
            for name, s, d in PROPOSAL if names is None or name in names]


def calibrated(k=2, margin=0.2, counts=(96,), names=None, alpha=0.05):
    return {'method': 'calibrated_paired', 'overall_alpha': alpha, 'confirmatory_contrasts': k,
            'noninferiority_margin': margin, 'required_discovery_gain': 0.0,
            'task_counts': list(counts), 'repetitions_per_task': 1, 'scenarios': scenarios(names)}


def searching(k=2, margin=0.2, multiple=6, largest=640, target=0.9):
    data = calibrated(k, margin, names=('H2',))
    del data['task_counts']
    data['search'] = {'scenario': 'H2', 'per_endpoint_target_power': target,
                      'task_multiple': multiple, 'max_tasks': largest}
    return data


def multinomial(n, b, c, gain, loss):
    """Independent outcome chance, written with integer binomial coefficients."""
    same = max(1.0 - gain - loss, 0.0)
    return math.comb(n, b) * math.comb(n - b, c) * gain ** b * loss ** c * same ** (n - b - c)


def published(x):
    """The proposal's three-decimal format."""
    return '<0.001' if x < 0.0005 else ('>0.999' if x > 0.9995 else f'{x:.3f}')


class CalibratedPairedTests(unittest.TestCase):
    def test_success_size_is_within_tail_on_the_grid_and_raised_no_further(self):
        for n, margin, tail in ((20, 0.1, 0.0125), (40, 0.2, 0.0125), (33, 0.15, 0.025), (60, 0.2, 0.05 / 14)):
            with self.subTest(n=n, margin=margin, tail=tail):
                test = power.calibrate_success_test(n, margin, tail)
                z = {(b, c): power.tango_statistic(n, b, c, -margin)
                     for b in range(n + 1) for c in range(n + 1 - b)}
                rejected = {o for o, value in z.items() if value > test['critical_value']}
                self.assertEqual(rejected, set(test['rejected']))
                top = (1 - margin) / 2

                def worst(region):
                    return max(math.fsum(multinomial(n, b, c, top * i / 60, top * i / 60 + margin)
                                         for b, c in region) for i in range(61))
                self.assertLessEqual(worst(rejected), tail)
                self.assertAlmostEqual(worst(rejected), test['worst_size'], places=12)
                self.assertGreaterEqual(test['critical_value'], test['nominal_critical_value'])
                if test['critical_value'] > test['nominal_critical_value']:
                    # Rejecting the outcomes at the critical value as well would
                    # break the tail, so the value was raised no further than needed.
                    self.assertGreater(test['worst_size_at_nominal'], tail)
                    self.assertGreater(worst({o for o, v in z.items() if v >= test['critical_value']}), tail)
                else:
                    self.assertLessEqual(test['worst_size_at_nominal'], tail)
        # The uncalibrated test exceeds its tail here, as the proposal reports.
        self.assertEqual(f"{power.calibrate_success_test(20, 0.1, 0.0125)['worst_size_at_nominal']:.4f}", '0.0214')

    def test_sign_test_keeps_its_size_exactly(self):
        tail = 0.0125
        critical = power.sign_test_critical_counts(60, tail)
        def upper(d, first):
            return Fraction(sum(math.comb(d, j) for j in range(first, d + 1)), 2 ** d)
        for d in range(1, 61):
            k = critical[d]
            if k is None:
                self.assertGreater(upper(d, d), Fraction(tail))
            else:
                self.assertLessEqual(upper(d, k), Fraction(tail))
                self.assertGreater(upper(d, k - 1), Fraction(tail))
        for n in (10, 30, 60):
            rejected = power.sign_test_rejected(n, critical)
            for discordance in (0.02, 0.1, 0.3, 0.6, 1.0):
                for gain_share in (0.5, 0.4, 0.1):  # 0.5 is the boundary gain = loss
                    gain, loss = discordance * gain_share, discordance * (1 - gain_share)
                    size = math.fsum(multinomial(n, b, c, gain, loss) for b, c in rejected)
                    self.assertLessEqual(size, tail + 1e-15)

    def test_hand_computed_small_cases(self):
        # With delta 0 Tango's statistic is McNemar's (b - c) / sqrt(b + c).
        self.assertAlmostEqual(power.tango_statistic(10, 5, 1, 0.0), 4 / math.sqrt(6), places=14)
        # Two tasks, one gain, delta -0.2: the restricted loss chance is 0.4 and
        # the variance is 2 (2 * 0.4 - 0.2 * 1.2) = 1.12.
        self.assertAlmostEqual(power.tango_statistic(2, 1, 0, -0.2), 1.4 / math.sqrt(1.12), places=14)
        # One task, margin 0.2: a gain scores sqrt(1.5), a tie 0.5, a loss -sqrt(2/3).
        self.assertAlmostEqual(power.tango_statistic(1, 1, 0, -0.2), math.sqrt(1.5), places=14)
        self.assertAlmostEqual(power.tango_statistic(1, 0, 0, -0.2), 0.5, places=14)
        self.assertAlmostEqual(power.tango_statistic(1, 0, 1, -0.2), -math.sqrt(2 / 3), places=14)
        # At tail 0.45 the normal value 0.126 rejects a gain or a tie. On the
        # boundary that happens with chance 1 - 0.2 - gain, worst 0.8 at gain 0.
        # Rejecting only a gain has chance gain, worst 0.4 at the grid's end, so
        # the critical value rises to the tie's 0.5.
        test = power.calibrate_success_test(1, 0.2, 0.45)
        self.assertAlmostEqual(test['nominal_critical_value'], NormalDist().inv_cdf(0.55), places=14)
        self.assertAlmostEqual(test['worst_size_at_nominal'], 0.8, places=14)
        self.assertAlmostEqual(test['critical_value'], 0.5, places=14)
        self.assertAlmostEqual(test['worst_size'], 0.4, places=14)
        self.assertEqual(test['rejected'], [(1, 0)])
        one = power.calculate(calibrated(k=1, counts=[1], names=('H2',), alpha=0.9))['results'][0]
        self.assertAlmostEqual(one['success_power'], 0.1, places=14)  # the chance of a gain
        self.assertEqual(one['discovery_power'], 0.0)  # 1/2 > 0.45, so no sign test rejects
        # Sign test at tail 0.0125: up to 6 discordant tasks never reject (1/64 >
        # 0.0125); 7, 8 and 9 need all gains; 10 needs 9 (11/1024 <= 0.0125).
        self.assertEqual(power.sign_test_critical_counts(10, 0.0125), [None] * 7 + [7, 8, 9, 9])
        data = calibrated(counts=[7], names=('H2',))
        data['scenarios'][0]['discovery'] = {'baseline': 0.3, 'gain': 0.5, 'loss': 0.1}
        self.assertAlmostEqual(power.calculate(data)['results'][0]['discovery_power'], 0.5 ** 7, places=15)

    def test_power_matches_independent_enumeration_of_task_sequences(self):
        n, margin, alpha = 5, 0.3, 0.1  # one contrast, tail 0.05
        data = calibrated(k=1, margin=margin, counts=[n], alpha=alpha)
        output = power.calculate(data)
        critical = output['calibration'][0]['critical_value']
        for row, scenario in zip(output['results'], data['scenarios']):
            expected = {'success': 0.0, 'discovery': 0.0}
            for endpoint in expected:
                gain, loss = scenario[endpoint]['gain'], scenario[endpoint]['loss']
                chance = {1: gain, -1: loss, 0: 1 - gain - loss}
                for sequence in itertools.product((1, 0, -1), repeat=n):
                    b, c = sequence.count(1), sequence.count(-1)
                    if endpoint == 'success':
                        reject = power.tango_statistic(n, b, c, -margin) > critical
                    else:
                        upper = Fraction(sum(math.comb(b + c, j) for j in range(b, b + c + 1)), 2 ** (b + c))
                        reject = b + c > 0 and upper <= Fraction(0.05)
                    if reject:
                        expected[endpoint] += math.prod(chance[v] for v in sequence)
            with self.subTest(scenario=row['scenario']):
                self.assertAlmostEqual(row['success_power'], expected['success'], places=13)
                self.assertAlmostEqual(row['discovery_power'], expected['discovery'], places=13)

    def test_power_rises_with_tasks_on_a_doubling_scale(self):
        # Published cells of the proposal's Result 2 table (H2, k = 2).
        cells = {0.2: (['0.218', '0.567', '0.934', '0.999'], ['0.107', '0.465', '0.850', '0.995']),
                 0.1: (['0.051', '0.153', '0.337', '0.681'], ['0.107', '0.465', '0.850', '0.995'])}
        for margin, (success, discovery) in cells.items():
            rows = power.calculate(calibrated(margin=margin, counts=[20, 40, 80, 160], names=('H2',)))['results']
            for name in ('success_power', 'discovery_power'):
                values = [row[name] for row in rows]
                self.assertEqual(values, sorted(set(values)), (margin, name))
            self.assertEqual([published(r['success_power']) for r in rows], success)
            self.assertEqual([published(r['discovery_power']) for r in rows], discovery)
        # Discrete tests are not monotone at every step: 112 tasks has less
        # success power than 110 (k = 6, margin 0.20).
        rows = power.calculate(calibrated(k=6, counts=[110, 112], names=('H2',)))['results']
        self.assertLess(rows[1]['success_power'], rows[0]['success_power'])

    def test_published_design_a_figures(self):
        # Sensitivity rows H1..H7 (success/discovery) of the proposal's
        # "Sizes, power and provider calls" section.
        def check(margin, n, row):
            output = power.calculate(calibrated(margin=margin, counts=[n]))
            got = '; '.join(f"{published(r['success_power'])}/{published(r['discovery_power'])}"
                            for r in output['results'])
            self.assertEqual(got, row)
            return output['calibration'][0], output['results'][1]
        test, h2 = check(0.2, 96, '0.999/0.405; 0.968/0.922; 0.790/0.998; >0.999/>0.999; '
                                  '>0.999/0.793; >0.999/>0.999; 0.404/0.007')
        self.assertEqual(published(h2['joint_power_lower_bound']), '0.890')
        self.assertEqual(f"{test['nominal_critical_value']:.3f}", '2.241')
        self.assertEqual(f"{test['critical_value']:.3f}", '2.280')
        self.assertEqual(f"{test['worst_size_at_nominal']:.4f}", '0.0135')
        self.assertEqual(f"{test['worst_size']:.4f}", '0.0121')
        _, h2 = check(0.1, 264, '0.994/0.917; 0.901/>0.999; 0.619/>0.999; >0.999/>0.999; '
                                '>0.999/0.999; >0.999/>0.999; 0.011/0.009')
        self.assertEqual(published(h2['joint_power_lower_bound']), '0.901')
        # The balanced search (multiples of 6) reproduces 264 at margin 0.10.
        self.assertEqual(power.calculate(searching(margin=0.1))['search']['smallest_tasks'], 264)
        # At margin 0.20 it finds 90, not the proposal's 96: the proposal scanned
        # steps of 4 with the uncalibrated critical value, found 92, and rounded
        # up to a multiple of 6. At 90 the calibrated method gives 0.945 / 0.900.
        found = power.calculate(searching(margin=0.2))
        self.assertEqual(found['search']['smallest_tasks'], 90)
        self.assertEqual([published(found['results'][0][x]) for x in ('success_power', 'discovery_power')],
                         ['0.945', '0.900'])
        budget = power._Budget(10 ** 9)
        critical = power.sign_test_critical_counts(100, 0.0125)
        def uncalibrated(n):
            plan = scenarios(('H2',))[0]
            region = power._success_region(n, 0.2, 0.0125, budget)['outcomes']
            return min(power._rejection_power(n, region, plan['success'], budget),
                       power._rejection_power(n, power.sign_test_rejected(n, critical), plan['discovery'], budget))
        self.assertEqual(next(n for n in range(20, 101, 4) if uncalibrated(n) >= 0.9), 92)

    def test_calibrated_inputs_are_refused(self):
        def change(edit):
            data = calibrated(counts=[20], names=('H2',))
            edit(data)
            return data
        cases = {
            'no counts or search': change(lambda d: d.pop('task_counts')),
            'both counts and search': change(lambda d: d.update(search=searching()['search'])),
            'unknown method': change(lambda d: d.update(method='bootstrap')),
            'unknown key': change(lambda d: d.update(extra=1)),
            'discovery gain': change(lambda d: d.update(required_discovery_gain=0.1)),
            'margin one': change(lambda d: d.update(noninferiority_margin=1.0)),
            'margin zero': change(lambda d: d.update(noninferiority_margin=0.0)),
            'tiny tail': change(lambda d: d.update(overall_alpha=1e-12)),
            'bad probability': change(lambda d: d['scenarios'][0]['success'].update(gain=0.6)),
            'unsorted counts': change(lambda d: d.update(task_counts=[40, 20])),
            'count too large': change(lambda d: d.update(task_counts=[1001])),
            'over the bound': calibrated(counts=[900, 1000]),  # about 69 million evaluations
            'stratified input': change(lambda d: (d.pop('task_counts'), d.pop('scenarios'),
                                                  d.pop('repetitions_per_task'), d.update(designs=[]))),
        }
        searches = {'unknown scenario': lambda s: s.update(scenario='H9'),
                    'target one': lambda s: s.update(per_endpoint_target_power=1.0),
                    'multiple zero': lambda s: s.update(task_multiple=0),
                    'multiple boolean': lambda s: s.update(task_multiple=True),
                    'max below multiple': lambda s: s.update(max_tasks=4),
                    'missing max': lambda s: s.pop('max_tasks')}
        for label, edit in searches.items():
            data = searching()
            edit(data['search'])
            cases['search: ' + label] = data
        for label, data in cases.items():
            with self.subTest(label), self.assertRaises(ValueError):
                power.calculate(data)
        path = Path(__file__).with_name('pilot-v2-power.py')
        run = subprocess.run([sys.executable, str(path)], input=json.dumps(cases['discovery gain']),
                             text=True, capture_output=True, check=False)
        self.assertEqual((run.returncode, run.stdout), (2, ''))
        self.assertIn('required_discovery_gain', run.stderr)

    def test_every_output_is_unfrozen_and_the_default_is_unchanged(self):
        path = Path(__file__).with_name('pilot-v2-power.py')
        not_found = searching(largest=30)
        for data in (calibrated(counts=[10, 20], names=('H1', 'H2')), searching(largest=120), not_found):
            run = subprocess.run([sys.executable, str(path)], input=json.dumps(data),
                                 text=True, capture_output=True, check=False)
            self.assertEqual(run.returncode, 0, run.stderr)
            output = json.loads(run.stdout)
            self.assertEqual(output['status'], 'sensitivity_only_not_frozen')
            self.assertEqual(output['method'], 'calibrated_paired')
            self.assertIs(output['execution_authorized'], False)
            self.assertEqual((output['provider_calls'], output['spend_usd']), (0, 0))
        self.assertEqual(output['search']['smallest_tasks'], None)
        self.assertEqual(output['results'], [])
        # No method field and an explicit conservative_bound give the same numbers.
        default = power.calculate(example())
        self.assertNotIn('method', default)
        explicit = power.calculate({**example(), 'method': 'conservative_bound'})
        self.assertEqual(explicit['results'], default['results'])
        self.assertEqual(explicit['method'], 'conservative_bound')
        self.assertIs(explicit['execution_authorized'], False)


if __name__ == '__main__':
    unittest.main()

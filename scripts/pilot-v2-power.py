#!/usr/bin/env python3
"""Unfrozen paired-task power sensitivity for pilot v2; no execution authority.

Read one JSON object from stdin and write one JSON object to stdout. The
pilot-v2 contract describes the inputs. Standard library only. Nothing here is
frozen: every output says sensitivity_only_not_frozen, execution_authorized
false, zero provider calls and zero spend.

There are two analysis methods.

conservative_bound is the default. It is used when the input has no "method"
field, or has "method": "conservative_bound". A contrast passes an endpoint only
if the observed mean paired difference minus sqrt(2 ln(1 / tail) / tasks) is
above the null boundary. The half-width ignores how variable the differences
are, so this method needs many tasks.

calibrated_paired is used only when the input says "method": "calibrated_paired".
It is the variance-based method of the 2026-11-20 sample proposal:

- Success safeguard: Tango's score test for a paired difference of proportions
  (Tango 1998, Statistics in Medicine 17:891-908), H0: gain - loss <= -margin.
  The critical value starts at the normal quantile for the tail. It is then
  raised, one observed statistic value at a time, until the largest rejection
  chance over a 61-point grid of discordance on the boundary gain - loss =
  -margin is within the tail. This controls size on the grid only; it is not a
  proof for every point of the null hypothesis.
- Improvement: the exact one-sided sign test (McNemar) on the discordant tasks,
  H0: gain - loss <= 0. It controls size exactly, so required_discovery_gain
  must be 0.
- Power: exact enumeration of every (gain count, loss count) outcome of the
  tasks, summed in floating point. Nothing is sampled, so there is no seed.
- Search: with a "search" object instead of "task_counts", the tool scans the
  multiples of a balance unit and reports the smallest one at which both
  endpoints of a named scenario reach a target power.
"""
import json
import math
from decimal import Decimal
from fractions import Fraction
from statistics import NormalDist
import sys


def _object(value, keys, name):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise ValueError(f"{name} must have exactly: {', '.join(keys)}")
    return value


def _number(value, name, lower, upper, *, strict_lower=False, strict_upper=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{name} must be finite")
    try:
        finite = math.isfinite(value)
    except OverflowError:
        finite = False
    if not finite:
        raise ValueError(f"{name} must be finite")
    if value < lower or value > upper or (strict_lower and value == lower) or (strict_upper and value == upper):
        raise ValueError(f"{name} out of range")
    return float(value)


def _integer(value, name, lower, upper):
    if type(value) is not int or not lower <= value <= upper:
        raise ValueError(f"{name} must be an integer in {lower}..{upper}")
    return value


def _endpoint(value, name):
    value = _object(value, ('baseline', 'gain', 'loss'), name)
    baseline = _number(value['baseline'], name + '.baseline', 0, 1)
    gain = _number(value['gain'], name + '.gain', 0, 1)
    loss = _number(value['loss'], name + '.loss', 0, 1)
    if Decimal(str(gain)) > Decimal(1) - Decimal(str(baseline)) or Decimal(str(loss)) > Decimal(str(baseline)):
        raise ValueError(f"{name}: gain/loss incompatible with baseline")
    return {'baseline': baseline, 'gain': gain, 'loss': loss}


def _block_endpoint(value, repetitions, name):
    if not isinstance(value, dict) or 'outcomes' not in value:
        return _endpoint(value, name)
    value = _object(value, ('outcomes',), name)
    if repetitions > 12:
        raise ValueError(f'{name}: explicit block distributions allow at most 12 repetitions')
    outcomes = value['outcomes']
    if not isinstance(outcomes, list) or not outcomes or len(outcomes) > 2 * repetitions + 1:
        raise ValueError(f'{name}: invalid block outcome count')
    seen = set()
    normalized = []
    total = Decimal(0)
    for outcome in outcomes:
        outcome = _object(outcome, ('difference_sum', 'probability'), name + '.outcome')
        difference = _integer(outcome['difference_sum'], name + '.difference_sum', -repetitions, repetitions)
        probability = _number(outcome['probability'], name + '.probability', 0, 1)
        if difference in seen:
            raise ValueError(f'{name}: duplicate block outcome')
        seen.add(difference)
        total += Decimal(str(probability))
        normalized.append({'difference_sum': difference, 'probability': probability})
    if total != Decimal(1):
        raise ValueError(f'{name}: block probabilities must sum exactly to 1')
    return {'outcomes': sorted(normalized, key=lambda x: x['difference_sum'])}


def _block_scale(strata, endpoint=None):
    endpoints = (endpoint,) if endpoint else ('success', 'discovery')
    denominators = [item['repetitions_per_task'] for item in strata
                    if any('outcomes' in item[name] for name in endpoints)]
    scale = math.lcm(*denominators) if denominators else 1
    if scale > 60:
        raise ValueError('block repetition common denominator exceeds 60')
    return scale


def block_distribution(strata, endpoint, scale):
    """Sum independent task-block means on an exact common integer grid."""
    probabilities = {0: 1.0}
    for item in strata:
        definition = item[endpoint]
        if 'outcomes' in definition:
            repetitions = item['repetitions_per_task']
            assert scale % repetitions == 0
            support = [(v['difference_sum'] * (scale // repetitions), v['probability'])
                       for v in definition['outcomes'] if v['probability'] > 0]
        else:
            gain, loss = definition['gain'], definition['loss']
            same = float(Decimal(1) - Decimal(str(gain)) - Decimal(str(loss)))
            support = [(step, chance) for step, chance in ((-scale, loss), (0, same), (scale, gain)) if chance > 0]
        for _ in range(item['tasks']):
            out = {}
            for offset, chance in probabilities.items():
                for step, weight in support:
                    key = offset + step
                    out[key] = out.get(key, 0.0) + chance * weight
            probabilities = out
    assert abs(math.fsum(probabilities.values()) - 1.0) < 1e-9
    return probabilities


def block_endpoint_power(strata, endpoint, null_upper, width):
    scale = _block_scale(strata, endpoint)
    count = sum(x['tasks'] for x in strata)
    distribution = block_distribution(strata, endpoint, scale)
    return math.fsum(p for offset, p in distribution.items() if offset / (count * scale) - width > null_upper)


def stratified_distribution(strata):
    """PMF of summed independent task differences, with per-stratum rates."""
    probabilities = [1.0]
    for tasks, gain, loss in strata:
        same = float(Decimal(1) - Decimal(str(gain)) - Decimal(str(loss)))
        for _ in range(tasks):
            out = [0.0] * (len(probabilities) + 2)
            for i, p in enumerate(probabilities):
                out[i] += p * loss
                out[i + 1] += p * same
                out[i + 2] += p * gain
            probabilities = out
    assert abs(math.fsum(probabilities) - 1.0) < 1e-9
    return probabilities


def difference_distribution(n, gain, loss):
    """PMF of sum of n independent identically distributed task differences."""
    return stratified_distribution([(n, gain, loss)])


def endpoint_power(n, endpoint, null_upper, width):
    distribution = difference_distribution(n, endpoint['gain'], endpoint['loss'])
    # A strict lower bound above the null boundary is required.
    return math.fsum(p for i, p in enumerate(distribution) if (i - n) / n - width > null_upper)


def calculate_stratified(spec):
    spec = _object(spec, ('overall_alpha', 'confirmatory_contrasts', 'noninferiority_margin',
                          'required_discovery_gain', 'designs'), 'input')
    alpha = _number(spec['overall_alpha'], 'overall_alpha', 0, 1, strict_lower=True, strict_upper=True)
    contrasts = _integer(spec['confirmatory_contrasts'], 'confirmatory_contrasts', 1, 1000)
    margin = _number(spec['noninferiority_margin'], 'noninferiority_margin', 0, 1, strict_lower=True)
    gain = _number(spec['required_discovery_gain'], 'required_discovery_gain', 0, 1, strict_upper=True)
    designs = spec['designs']
    if not isinstance(designs, list) or not designs or len(designs) > 16:
        raise ValueError('designs must contain 1..16 entries')
    names = set()
    normalized = []
    for design in designs:
        design = _object(design, ('name', 'strata'), 'design')
        name = design['name']
        if not isinstance(name, str) or not name.strip() or len(name) > 100 or name in names:
            raise ValueError('design names must be unique nonblank strings of at most 100 characters')
        names.add(name)
        strata = design['strata']
        if not isinstance(strata, list) or not strata or len(strata) > 32:
            raise ValueError('strata must contain 1..32 entries')
        stratum_names = set()
        items = []
        for stratum in strata:
            stratum = _object(stratum, ('name', 'tasks', 'repetitions_per_task', 'success', 'discovery'), 'stratum')
            label = stratum['name']
            if not isinstance(label, str) or not label.strip() or len(label) > 100 or label in stratum_names:
                raise ValueError('stratum names must be unique nonblank strings of at most 100 characters')
            stratum_names.add(label)
            repetitions = _integer(stratum['repetitions_per_task'], 'repetitions_per_task', 1, 1000)
            items.append({'name': label, 'tasks': _integer(stratum['tasks'], 'tasks', 1, 1000),
                          'repetitions_per_task': repetitions,
                          'success': _block_endpoint(stratum['success'], repetitions, name + '.' + label + '.success'),
                          'discovery': _block_endpoint(stratum['discovery'], repetitions, name + '.' + label + '.discovery')})
        if sum(x['tasks'] for x in items) > 1000:
            raise ValueError('a design may contain at most 1000 independent tasks')
        normalized.append({'name': name, 'strata': items})
    estimated_operations = 0
    for design in normalized:
        strata = design['strata']
        scale = _block_scale(strata)
        explicit = any('outcomes' in x[endpoint] for x in strata for endpoint in ('success', 'discovery'))
        support = max((len(x[endpoint]['outcomes']) if 'outcomes' in x[endpoint] else 3)
                      for x in strata for endpoint in ('success', 'discovery')) if explicit else 1
        estimated_operations += 4 * sum(x['tasks'] for x in strata) ** 2 * scale * support
    if estimated_operations > 12_000_000:
        raise ValueError('requested calculation exceeds operation bound')
    tail_alpha = alpha / (2 * contrasts)
    if tail_alpha == 0.0:
        raise ValueError('per-endpoint alpha underflows')
    table = []
    for design in normalized:
        strata = design['strata']
        n = sum(x['tasks'] for x in strata)
        width = math.sqrt(2 * -math.log(tail_alpha) / n)
        powers = {}
        for endpoint, boundary in (('success', -margin), ('discovery', gain)):
            if any('outcomes' in x[endpoint] for x in strata):
                powers[endpoint] = block_endpoint_power(strata, endpoint, boundary, width)
            else:
                distribution = stratified_distribution([(x['tasks'], x[endpoint]['gain'], x[endpoint]['loss']) for x in strata])
                powers[endpoint] = math.fsum(p for i, p in enumerate(distribution) if (i - n) / n - width > boundary)
        stratum_results = []
        for item in strata:
            count = item['tasks']
            local_width = math.sqrt(2 * -math.log(tail_alpha) / count)
            local_success = (block_endpoint_power([item], 'success', -margin, local_width)
                             if 'outcomes' in item['success'] else endpoint_power(count, item['success'], -margin, local_width))
            local_discovery = (block_endpoint_power([item], 'discovery', gain, local_width)
                               if 'outcomes' in item['discovery'] else endpoint_power(count, item['discovery'], gain, local_width))
            stratum_results.append({'name': item['name'], 'tasks': count,
                                    'repetitions_per_task': item['repetitions_per_task'],
                                    'success_power': local_success, 'discovery_power': local_discovery,
                                    'joint_power_lower_bound': max(0.0, local_success + local_discovery - 1.0),
                                    'one_sided_half_width': local_width})
        table.append({'design': design['name'], 'independent_tasks': n,
                      'strata': stratum_results,
                      'success_power': powers['success'], 'discovery_power': powers['discovery'],
                      'joint_power_lower_bound': max(0.0, powers['success'] + powers['discovery'] - 1.0),
                      'one_sided_half_width': width})
    return {'status': 'sensitivity_only_not_frozen', 'inputs': {**spec, 'designs': normalized},
            'per_endpoint_tail_alpha': tail_alpha, 'results': table,
            'assumptions': ['task blocks are independent across and within strata, identically distributed within each stratum',
                            'strata have fixed task counts and contribute in proportion to task count',
                            'stratum-specific claims require those claims to be included in confirmatory_contrasts',
                            'each endpoint is a paired binary outcome',
                            'legacy gain/loss endpoints assume perfect correlation among repetitions; explicit block distributions supply a different assumption',
                            'each task block is independent; repetitions are never independent task blocks',
                            'unknown dependence between endpoints; joint power is a union-bound lower bound',
                            'no cost, latency, host or rights qualification'],
            'execution_authorized': False, 'provider_calls': 0, 'spend_usd': 0}


CONSERVATIVE = 'conservative_bound'
CALIBRATED = 'calibrated_paired'
# Points on the boundary grid used to calibrate the success test, as in the
# 2026-11-20 proposal: gain runs evenly from 0 to (1 - margin) / 2.
GRID_POINTS = 61
# Upper limit on outcome evaluations (one Tango statistic or one outcome
# probability each) in one calibrated request: about 30 seconds on the owner's
# Mac. One task count of 1,000 with 16 scenarios fits.
CALIBRATED_OPERATION_BOUND = 60_000_000
# Below this tail the floating-point sums are too coarse to compare with it.
SMALLEST_CALIBRATED_TAIL = 1e-9


class _Budget:
    """Counts outcome evaluations and refuses once the bound is spent."""

    def __init__(self, limit):
        self.left = limit

    def spend(self, count):
        self.left -= count
        if self.left < 0:
            raise ValueError('requested calculation exceeds operation bound')


def outcome_probabilities(n, outcomes, gain, loss, same):
    """Multinomial chance of each (gain count, loss count) outcome of n tasks.

    gain, loss and same are the per-task chances that only the treatment passes,
    only the control passes, or both runs agree. They must sum to one.
    """
    log_factorial = [math.lgamma(i + 1) for i in range(n + 1)]
    log_gain, log_loss, log_same = (math.log(p) if p > 0 else -math.inf for p in (gain, loss, same))
    top = log_factorial[n]
    exp = math.exp
    result = []
    for b, c in outcomes:
        rest = n - b - c
        value = top - log_factorial[b] - log_factorial[c] - log_factorial[rest]
        # A zero chance raised to a zero count is one, so skip zero counts.
        if b:
            value += b * log_gain
        if c:
            value += c * log_loss
        if rest:
            value += rest * log_same
        result.append(exp(value))
    return result


def tango_statistic(n, b, c, delta):
    """Tango's (1998) score statistic for H0: gain - loss = delta.

    b tasks gained and c tasks lost out of n. The variance uses the maximum
    likelihood loss chance under H0, the larger root of Tango's quadratic.
    Large values favour gain - loss > delta. A zero variance returns -inf, so
    that outcome is never rejected. The arithmetic follows the proposal's
    script operation for operation, so the values agree bit for bit.
    """
    a = 2.0 * n
    linear = -b - c + (2.0 * n - b + c) * delta
    constant = -c * delta * (1.0 - delta)
    root = (math.sqrt(max(linear * linear - 4.0 * a * constant, 0.0)) - linear) / (2.0 * a)
    variance = n * (2.0 * root + delta * (1.0 - delta))
    if not variance > 0:
        return -math.inf
    return (b - c - n * delta) / math.sqrt(variance)


def boundary_grid(margin):
    """The calibration grid: (gain, loss, same) with gain - loss = -margin."""
    top = (1 - margin) / 2
    last = GRID_POINTS - 1
    # same is written directly so that the last point has exactly zero.
    return [(top * i / last, top * i / last + margin, (1 - margin) * (last - i) / last)
            for i in range(GRID_POINTS)]


def _success_region(n, margin, tail, budget):
    """Outcomes that the uncalibrated Tango test rejects.

    Returns the normal critical value, the rejected outcomes in decreasing order
    of the statistic, their statistics, and the end index of each run of equal
    statistics (outcomes with equal statistics are rejected together).
    """
    nominal = NormalDist().inv_cdf(1 - tail)
    budget.spend((n + 1) * (n + 2) // 2)
    scored = []
    for b in range(n + 1):
        for c in range(n + 1 - b):
            z = tango_statistic(n, b, c, -margin)
            if z > nominal:
                scored.append((z, b, c))
    scored.sort(reverse=True)
    statistics = [z for z, _, _ in scored]
    ends = [i + 1 for i in range(len(scored)) if i + 1 == len(scored) or scored[i + 1][0] != scored[i][0]]
    return {'nominal': nominal, 'outcomes': [(b, c) for _, b, c in scored],
            'statistics': statistics, 'ends': ends}


def _calibrate(n, margin, tail, region, budget):
    """Raise the critical value until the worst boundary size is within the tail.

    worst[g] is the largest chance, over the grid, of rejecting every outcome in
    tie groups 0..g. It never decreases with g. The calibrated test rejects the
    longest prefix of groups whose worst chance is within the tail, so the
    critical value is the statistic of the first group left out. This is the
    smallest observed statistic at or above the normal quantile that keeps the
    grid size within the tail, as in the proposal's script.
    """
    outcomes, ends = region['outcomes'], region['ends']
    budget.spend(GRID_POINTS * len(outcomes))
    worst = [0.0] * len(ends)
    for gain, loss, same in boundary_grid(margin):
        running = 0.0
        group = 0
        for index, chance in enumerate(outcome_probabilities(n, outcomes, gain, loss, same), 1):
            running += chance
            if index == ends[group]:
                if running > worst[group]:
                    worst[group] = running
                group += 1
    nominal_size = worst[-1] if worst else 0.0
    if nominal_size <= tail:
        return {'nominal_critical_value': region['nominal'], 'critical_value': region['nominal'],
                'worst_size_at_nominal': nominal_size, 'worst_size': nominal_size,
                'rejected': outcomes}
    kept = next(group for group, size in enumerate(worst) if size > tail)
    count = ends[kept - 1] if kept else 0
    return {'nominal_critical_value': region['nominal'],
            'critical_value': region['statistics'][ends[kept] - 1],
            'worst_size_at_nominal': nominal_size, 'worst_size': worst[kept - 1] if kept else 0.0,
            'rejected': outcomes[:count]}


def calibrate_success_test(n, margin, tail, budget=None):
    """The calibrated Tango non-inferiority test for n tasks.

    Returns both critical values, the worst grid size at each, and the list of
    rejected (gain count, loss count) outcomes.
    """
    budget = budget or _Budget(CALIBRATED_OPERATION_BOUND)
    return _calibrate(n, margin, tail, _success_region(n, margin, tail, budget), budget)


def sign_test_critical_counts(largest, tail):
    """Smallest rejected gain count for each discordant total 0..largest.

    The exact one-sided sign test rejects b gains out of d discordant tasks when
    P(Binomial(d, 1/2) >= b) <= tail. The comparison uses integer binomial sums
    and the tail as an exact fraction, so it has no rounding. None means that d
    discordant tasks can never be rejected.
    """
    limit = Fraction(tail)
    critical = [None]
    for d in range(1, largest + 1):
        allowed = limit * (1 << d)
        upper, term, best = 0, 1, None
        for k in range(d, -1, -1):
            upper += term  # term is C(d, k)
            if upper > allowed:
                break
            best = k
            term = term * k // (d - k + 1)
        critical.append(best)
    return critical


def sign_test_rejected(n, critical):
    """The (gain count, loss count) outcomes that the sign test rejects."""
    return [(b, d - b) for d in range(1, n + 1) if critical[d] is not None
            for b in range(critical[d], d + 1)]


def _rejection_power(n, outcomes, endpoint, budget):
    """Chance that n tasks with the endpoint's gain and loss land in outcomes."""
    budget.spend(len(outcomes))
    same = float(Decimal(1) - Decimal(str(endpoint['gain'])) - Decimal(str(endpoint['loss'])))
    return math.fsum(outcome_probabilities(n, outcomes, endpoint['gain'], endpoint['loss'], same))


def _calibrated_scenarios(value):
    """The default method's scenario checks, kept separate so its path is untouched."""
    if not isinstance(value, list) or not value or len(value) > 16:
        raise ValueError('scenarios must contain 1..16 entries')
    names = set()
    normalized = []
    for scenario in value:
        scenario = _object(scenario, ('name', 'success', 'discovery'), 'scenario')
        name = scenario['name']
        if not isinstance(name, str) or not name.strip() or len(name) > 100 or name in names:
            raise ValueError('scenario names must be unique nonblank strings of at most 100 characters')
        names.add(name)
        normalized.append({'name': name, 'success': _endpoint(scenario['success'], name + '.success'),
                           'discovery': _endpoint(scenario['discovery'], name + '.discovery')})
    return normalized


def calculate_calibrated(spec):
    """The calibrated_paired method: a table over task_counts, or a search."""
    if not isinstance(spec, dict) or ('task_counts' in spec) == ('search' in spec):
        raise ValueError('calibrated_paired input needs exactly one of task_counts or search')
    mode = 'task_counts' if 'task_counts' in spec else 'search'
    spec = _object(spec, ('method', 'overall_alpha', 'confirmatory_contrasts', 'noninferiority_margin',
                          'required_discovery_gain', 'repetitions_per_task', 'scenarios', mode), 'input')
    alpha = _number(spec['overall_alpha'], 'overall_alpha', 0, 1, strict_lower=True, strict_upper=True)
    contrasts = _integer(spec['confirmatory_contrasts'], 'confirmatory_contrasts', 1, 1000)
    # A margin of 1 leaves only gain 0, loss 1 in H0, which has no variance.
    margin = _number(spec['noninferiority_margin'], 'noninferiority_margin', 0, 1,
                     strict_lower=True, strict_upper=True)
    if _number(spec['required_discovery_gain'], 'required_discovery_gain', 0, 1, strict_upper=True) != 0:
        raise ValueError('calibrated_paired needs required_discovery_gain 0: the sign test tests no net gain')
    _integer(spec['repetitions_per_task'], 'repetitions_per_task', 1, 1000)
    scenarios = _calibrated_scenarios(spec['scenarios'])
    # Bonferroni over both one-sided tests of every declared contrast, as in
    # the default method.
    tail = alpha / (2 * contrasts)
    if tail < SMALLEST_CALIBRATED_TAIL:
        raise ValueError(f'per-endpoint alpha below {SMALLEST_CALIBRATED_TAIL} is not supported')
    budget = _Budget(CALIBRATED_OPERATION_BOUND)

    def summary(n, test):
        return {'independent_tasks': n, 'nominal_critical_value': test['nominal_critical_value'],
                'critical_value': test['critical_value'],
                'worst_size_at_nominal': test['worst_size_at_nominal'], 'worst_size': test['worst_size']}

    def rows(n, test, critical):
        discovery_rejected = sign_test_rejected(n, critical)
        table = []
        for scenario in scenarios:
            success = _rejection_power(n, test['rejected'], scenario['success'], budget)
            discovery = _rejection_power(n, discovery_rejected, scenario['discovery'], budget)
            table.append({'scenario': scenario['name'], 'independent_tasks': n,
                          'success_power': success, 'discovery_power': discovery,
                          'joint_power_lower_bound': max(0.0, success + discovery - 1.0)})
        return table

    inputs = {**spec, 'scenarios': scenarios}
    calibration, table, search_result = [], [], None
    if mode == 'task_counts':
        counts = spec['task_counts']
        if not isinstance(counts, list) or not counts or len(counts) > 20:
            raise ValueError('task_counts must contain 1..20 entries')
        counts = [_integer(x, 'task count', 1, 1000) for x in counts]
        if counts != sorted(set(counts)):
            raise ValueError('task_counts must be strictly increasing')
        # Refuse before any work if the request is plainly too large.
        estimate = sum((n + 1) * (n + 2) // 2 * (1 + GRID_POINTS + 2 * len(scenarios)) for n in counts)
        if estimate > CALIBRATED_OPERATION_BOUND:
            raise ValueError('requested calculation exceeds operation bound')
        critical = sign_test_critical_counts(counts[-1], tail)
        for n in counts:
            test = calibrate_success_test(n, margin, tail, budget)
            calibration.append(summary(n, test))
            table.extend(rows(n, test, critical))
    else:
        search = _object(spec['search'], ('scenario', 'per_endpoint_target_power', 'task_multiple', 'max_tasks'),
                         'search')
        plan = next((x for x in scenarios if x['name'] == search['scenario']), None)
        if plan is None:
            raise ValueError('search.scenario must name one of the scenarios')
        target = _number(search['per_endpoint_target_power'], 'search.per_endpoint_target_power', 0, 1,
                         strict_lower=True, strict_upper=True)
        multiple = _integer(search['task_multiple'], 'search.task_multiple', 1, 1000)
        largest = _integer(search['max_tasks'], 'search.max_tasks', multiple, 1000)
        inputs['search'] = {'scenario': plan['name'], 'per_endpoint_target_power': target,
                            'task_multiple': multiple, 'max_tasks': largest}
        critical = sign_test_critical_counts(largest, tail)
        found, checked = None, []
        for n in range(multiple, largest + 1, multiple):
            checked.append(n)
            # Cheap checks first. Calibration only removes rejected outcomes, so
            # the uncalibrated success power is an upper limit on the calibrated one.
            if _rejection_power(n, sign_test_rejected(n, critical), plan['discovery'], budget) < target:
                continue
            region = _success_region(n, margin, tail, budget)
            if _rejection_power(n, region['outcomes'], plan['success'], budget) < target:
                continue
            test = _calibrate(n, margin, tail, region, budget)
            if _rejection_power(n, test['rejected'], plan['success'], budget) < target:
                continue
            found = n
            calibration.append(summary(n, test))
            table = rows(n, test, critical)
            break
        search_result = {**inputs['search'], 'smallest_tasks': found, 'found': found is not None,
                         'task_counts_checked': len(checked),
                         'largest_task_count_checked': checked[-1] if checked else None}
    result = {'status': 'sensitivity_only_not_frozen', 'method': CALIBRATED, 'inputs': inputs,
              'per_endpoint_tail_alpha': tail,
              'success_test': 'Tango (1998) score test for gain - loss <= -margin, critical value raised '
                              f'until the worst size over a {GRID_POINTS}-point boundary grid is within the tail',
              'improvement_test': 'exact one-sided sign test (McNemar) on discordant tasks for gain - loss <= 0',
              'calibration': calibration, 'results': table}
    if search_result is not None:
        result['search'] = search_result
    result.update({
        'assumptions': ['independent, identically distributed task blocks within each scenario',
                        'each endpoint is a paired binary outcome',
                        'perfect correlation among repetitions of the same task; repetitions add no independent blocks',
                        f'the success test keeps its size within the tail only on the {GRID_POINTS}-point grid of '
                        'the margin boundary; this is a grid check, not a proof over the whole null hypothesis',
                        'the sign test keeps its size within the tail exactly, for every gain <= loss',
                        'power is exact enumeration of all outcomes, summed in floating point; nothing is sampled',
                        'power is not monotone in tasks for discrete tests; a search result is the first count '
                        'that reaches the target, and some larger counts may not',
                        'unknown dependence between endpoints; joint power is a union-bound lower bound',
                        'no stratification, cost, latency, host or rights qualification'],
        'execution_authorized': False, 'provider_calls': 0, 'spend_usd': 0})
    return result


def calculate(spec):
    if isinstance(spec, dict) and 'method' in spec:
        method = spec['method']
        if method == CALIBRATED:
            return calculate_calibrated(spec)
        if method != CONSERVATIVE:
            raise ValueError(f"method must be '{CONSERVATIVE}' or '{CALIBRATED}'")
        # The explicit default: identical to leaving the field out, plus a label.
        result = calculate({key: value for key, value in spec.items() if key != 'method'})
        result['inputs'] = {'method': CONSERVATIVE, **result['inputs']}
        result['method'] = CONSERVATIVE
        return result
    if isinstance(spec, dict) and 'designs' in spec:
        return calculate_stratified(spec)
    spec = _object(spec, ('overall_alpha', 'confirmatory_contrasts', 'noninferiority_margin',
                          'required_discovery_gain', 'task_counts', 'repetitions_per_task', 'scenarios'), 'input')
    alpha = _number(spec['overall_alpha'], 'overall_alpha', 0, 1, strict_lower=True, strict_upper=True)
    contrasts = _integer(spec['confirmatory_contrasts'], 'confirmatory_contrasts', 1, 1000)
    margin = _number(spec['noninferiority_margin'], 'noninferiority_margin', 0, 1, strict_lower=True)
    gain = _number(spec['required_discovery_gain'], 'required_discovery_gain', 0, 1, strict_upper=True)
    repetitions = _integer(spec['repetitions_per_task'], 'repetitions_per_task', 1, 1000)
    counts = spec['task_counts']
    if not isinstance(counts, list) or not counts or len(counts) > 20:
        raise ValueError('task_counts must contain 1..20 entries')
    counts = [_integer(x, 'task count', 1, 1000) for x in counts]
    if counts != sorted(set(counts)):
        raise ValueError('task_counts must be strictly increasing')
    scenarios = spec['scenarios']
    if not isinstance(scenarios, list) or not scenarios or len(scenarios) > 16:
        raise ValueError('scenarios must contain 1..16 entries')
    if 2 * len(scenarios) * sum(n * n for n in counts) > 8_000_000:
        raise ValueError('requested calculation exceeds operation bound')
    names = set()
    normalized = []
    for scenario in scenarios:
        scenario = _object(scenario, ('name', 'success', 'discovery'), 'scenario')
        name = scenario['name']
        if not isinstance(name, str) or not name.strip() or len(name) > 100 or name in names:
            raise ValueError('scenario names must be unique nonblank strings of at most 100 characters')
        names.add(name)
        normalized.append({'name': name, 'success': _endpoint(scenario['success'], name + '.success'),
                           'discovery': _endpoint(scenario['discovery'], name + '.discovery')})
    # Bonferroni over both one-sided bounds for every declared contrast.
    tail_alpha = alpha / (2 * contrasts)
    if tail_alpha == 0.0:
        raise ValueError('per-endpoint alpha underflows')
    table = []
    for scenario in normalized:
        for n in counts:
            width = math.sqrt(2 * -math.log(tail_alpha) / n)
            success = endpoint_power(n, scenario['success'], -margin, width)
            discovery = endpoint_power(n, scenario['discovery'], gain, width)
            table.append({'scenario': scenario['name'], 'independent_tasks': n,
                          'success_power': success, 'discovery_power': discovery,
                          'joint_power_lower_bound': max(0.0, success + discovery - 1.0),
                          'one_sided_half_width': width})
    return {'status': 'sensitivity_only_not_frozen', 'inputs': {**spec, 'scenarios': normalized},
            'per_endpoint_tail_alpha': tail_alpha, 'results': table,
            'assumptions': ['independent, identically distributed task blocks within each scenario',
                            'each endpoint is a paired binary outcome',
                            'perfect correlation among repetitions of the same task; repetitions add no independent blocks',
                            'unknown dependence between endpoints; joint power is a union-bound lower bound',
                            'no stratification, cost, latency, host or rights qualification'],
            'execution_authorized': False, 'provider_calls': 0, 'spend_usd': 0}


def main():
    try:
        payload = sys.stdin.read(1_000_001)
        if len(payload) > 1_000_000:
            raise ValueError('input exceeds 1 MB')
        def unique_object(pairs):
            result = {}
            for key, value in pairs:
                if key in result:
                    raise ValueError(f'duplicate key: {key}')
                result[key] = value
            return result
        def reject_constant(value):
            raise ValueError(f'non-finite constant: {value}')
        result = calculate(json.loads(payload, object_pairs_hook=unique_object, parse_constant=reject_constant))
    except (ValueError, TypeError, json.JSONDecodeError) as error:
        print(f'pilot-v2-power: {error}', file=sys.stderr)
        return 2
    json.dump(result, sys.stdout, indent=2, allow_nan=False)
    print()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

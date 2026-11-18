#!/usr/bin/env python3
"""Unfrozen, conservative paired-task power sensitivity; no execution authority.

Read one JSON object from stdin and write one JSON object to stdout. The method
and required inputs are described in the pilot-v2 contract. Standard library only.
"""
import json
import math
from decimal import Decimal
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


def calculate(spec):
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

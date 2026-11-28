#!/usr/bin/env python3
"""SR-01 qualification for Điểm Vink V1 (#600, REP-S01).

Deterministic, stdlib-only, synthetic data only (Q24). Run from the repository root:

    python3 -I services/vink-app/bench/reputation-sr01/qualify.py > results.json
    python3 -I services/vink-app/bench/reputation-sr01/qualify.py --vectors \
        > services/vink-app/behavior/reputation/formula-vectors.json

The script prints one JSON document:

- golden: exact rating vectors computed with 60-digit decimal arithmetic. The canonical score is
  `score_micros = floor(R * 10^6)` and the display rating is the half-up integer of R, which is
  also `floor((score_micros + 500_000) / 10^6)` (proved below for every vector).
- growth: Q28 fixed-unit growth in tenths, G_tenths = 24 * dI + 16 * dC.
- precision: how often an IEEE-754 f64 implementation (log1p / log) disagrees with the exact
  canonical score on a dense grid, and the smallest distance of R * 10^6 to an integer.
- distribution: rating percentiles and Top 100 composition over seeded synthetic populations.
- sensitivity: rank agreement of alternative k / reference against the V1 candidate k=10, ref=100.

Nothing here reads user data, fits parameters to real behaviour or measures PostgreSQL.
"""

import json
import math
import random
from decimal import ROUND_FLOOR, Decimal, getcontext

getcontext().prec = 60

BASE = Decimal(1000)
SCALE = Decimal(400)
W_IMPACT = Decimal("0.6")
W_CONTRIB = Decimal("0.4")
K = Decimal(10)
REFERENCE = Decimal(100)
MICROS = Decimal(1_000_000)
BOUND = 2**40  # checked domain bound for I and C (exclusive)


def power_of_eleven(x):
    """k when x + 10 = 10 * 11^k: f(x) = k exactly, which a finite-precision ln can miss."""
    a = x + 10
    if a % 10:
        return None
    m, k = a // 10, 0
    while m % 11 == 0:
        m, k = m // 11, k + 1
    return k if m == 1 else None


def f_exact(x, k=K, reference=REFERENCE):
    if k == K and reference == REFERENCE:
        exact = power_of_eleven(int(x))
        if exact is not None:
            return Decimal(exact)
    return (1 + Decimal(x) / k).ln() / (1 + reference / k).ln()


def rating_exact(i, c, k=K, reference=REFERENCE, w=W_IMPACT):
    return BASE + SCALE * (w * f_exact(i, k, reference) + (1 - w) * f_exact(c, k, reference))


def floor_int(value):
    return int(value.to_integral_value(rounding=ROUND_FLOOR))


def canonical(i, c):
    r = rating_exact(i, c)
    micros = floor_int(r * MICROS)
    display = floor_int(r + Decimal("0.5"))
    assert display == (micros + 500_000) // 1_000_000, (i, c)
    return r, micros, display


def golden():
    pairs = [
        (0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (10, 0), (0, 10), (10, 5), (5, 10),
        (100, 0), (0, 100), (100, 100), (300, 20), (20, 300), (1000, 600), (1000, 0),
        (0, 300), (1200, 0), (0, 1200), (1200, 1200), (13300, 13300), (99, 101),
        (1_000_000, 1_000_000), (BOUND - 1, 0), (0, BOUND - 1), (BOUND - 1, BOUND - 1),
    ]
    # Pairs whose fractional part lies closest to .5 within a small grid: rounding edge vectors.
    near_half = []
    for i in range(0, 400):
        fi = f_exact(i)
        for c in range(0, 400):
            r = BASE + SCALE * (W_IMPACT * fi + W_CONTRIB * f_exact(c))
            frac = r - floor_int(r)
            near_half.append((abs(frac - Decimal("0.5")), i, c))
    near_half.sort()
    pairs += [(i, c) for _, i, c in near_half[:6]]
    out = []
    for i, c in pairs:
        r, micros, display = canonical(i, c)
        out.append({
            "impact": i,
            "contribution": c,
            "rating_exact_30": format(r.quantize(Decimal(10) ** -30), "f"),
            "score_micros": micros,
            "display": display,
        })
    return out


def growth():
    cases = [
        ("one Thấm", 1, 0), ("one Ưng", 0, 1), ("withdraw active Thấm", -1, 0),
        ("withdraw active Ưng", 0, -1), ("10 Thấm + 10 Ưng", 10, 10),
        ("3 Thấm - 2 Ưng", 3, -2), ("2 Thấm - 3 Ưng", 2, -3), ("bound", BOUND - 1, BOUND - 1),
    ]
    return [
        {"case": name, "d_impact": di, "d_contribution": dc, "g_tenths": 24 * di + 16 * dc}
        for name, di, dc in cases
    ]


def precision(limit=1000):
    ln11 = math.log(11.0)
    f64 = [math.log1p(x / 10.0) / ln11 for x in range(limit)]
    exact = [f_exact(x) for x in range(limit)]
    exact_i = [BASE + SCALE * W_IMPACT * e for e in exact]
    exact_c = [SCALE * W_CONTRIB * e for e in exact]
    micros_mismatch = 0
    display_mismatch = 0
    exact_integers = 0
    min_gap = None
    min_gap_at = None
    for i in range(limit):
        for c in range(limit):
            r = exact_i[i] + exact_c[c]
            scaled = r * MICROS
            micros = floor_int(scaled)
            gap = min(scaled - micros, micros + 1 - scaled)
            if gap < Decimal(10) ** -40:
                # R is an integer only when f(I) and f(C) are integers (I, C in {0, 100, 1200}).
                exact_integers += 1
            elif min_gap is None or gap < min_gap:
                min_gap, min_gap_at = gap, (i, c)
            approx = 1000.0 + 400.0 * (0.6 * f64[i] + 0.4 * f64[c])
            if math.floor(approx * 1_000_000.0) != micros:
                micros_mismatch += 1
            if math.floor(approx + 0.5) != floor_int(r + Decimal("0.5")):
                display_mismatch += 1
    return {
        "grid": f"0..{limit - 1} x 0..{limit - 1}",
        "pairs": limit * limit,
        "f64_score_micros_mismatch": micros_mismatch,
        "f64_display_mismatch": display_mismatch,
        "exact_integer_ratings": exact_integers,
        "min_nonzero_distance_of_R_times_1e6_to_integer": format(min_gap, ".3e"),
        "min_distance_at": min_gap_at,
    }


def population(seed, n=20_000):
    """Seeded heavy-tailed synthetic members: most have no signal, a few dominate."""
    rng = random.Random(seed)
    members = []
    for idx in range(n):
        roll = rng.random()
        if roll < 0.55:
            i, c = 0, 0
        elif roll < 0.70:  # comment-only (Q11)
            i, c = 0, int(rng.paretovariate(1.3))
        elif roll < 0.85:  # writing-only
            i, c = int(rng.paretovariate(1.2)), 0
        else:  # both, loosely correlated
            base = rng.paretovariate(1.1)
            i = int(base * rng.uniform(0.5, 2.0))
            c = int(base * rng.uniform(0.2, 1.5))
        members.append((idx, min(i, 50_000), min(c, 50_000)))
    return members


def ranked(members, k, reference, w=W_IMPACT):
    cache = {}

    def f(x):
        if x not in cache:
            cache[x] = float(f_exact(x, k, reference))
        return cache[x]

    scored = [(1000 + 400 * (float(w) * f(i) + (1 - float(w)) * f(c)), idx, i, c)
              for idx, i, c in members]
    # canonical order: score desc, then id asc
    scored.sort(key=lambda row: (-row[0], row[1]))
    return scored


def spearman(order_a, order_b):
    pos_b = {idx: rank for rank, idx in enumerate(order_b)}
    n = len(order_a)
    d2 = sum((rank - pos_b[idx]) ** 2 for rank, idx in enumerate(order_a))
    return 1 - 6 * d2 / (n * (n * n - 1))


def distribution_and_sensitivity():
    seeds = [600, 601, 602]
    variants = [(10, 100), (5, 100), (20, 100), (10, 50), (10, 200)]
    dist = []
    sens = []
    for seed in seeds:
        members = population(seed)
        base_rank = ranked(members, K, REFERENCE)
        scores = sorted(row[0] for row in base_rank)
        pct = lambda p: round(scores[min(len(scores) - 1, int(p * len(scores)))])
        eligible = [row for row in base_rank if row[2] or row[3]]
        top = eligible[:100]
        dist.append({
            "seed": seed,
            "members": len(members),
            "baseline_only_share": round(sum(1 for r in base_rank if not (r[2] or r[3])) / len(members), 3),
            "p50": pct(0.5), "p90": pct(0.9), "p99": pct(0.99), "max": round(scores[-1]),
            "top100_comment_only": sum(1 for r in top if r[2] == 0),
            "top100_writing_only": sum(1 for r in top if r[3] == 0),
        })
        base_order = [row[1] for row in eligible]
        base_top = set(base_order[:100])
        for k, ref in variants[1:]:
            alt = [row for row in ranked(members, Decimal(k), Decimal(ref)) if row[2] or row[3]]
            alt_order = [row[1] for row in alt]
            sens.append({
                "seed": seed, "k": k, "reference": ref,
                "spearman_eligible": round(spearman(base_order, alt_order), 4),
                "top100_overlap": len(base_top & set(alt_order[:100])),
            })
        for w in (Decimal("0.5"), Decimal("0.7")):
            alt = [row for row in ranked(members, K, REFERENCE, w) if row[2] or row[3]]
            alt_order = [row[1] for row in alt]
            sens.append({
                "seed": seed, "weight_impact": str(w),
                "spearman_eligible": round(spearman(base_order, alt_order), 4),
                "top100_overlap": len(base_top & set(alt_order[:100])),
            })
    return dist, sens


def shape_checks(limit=10_001):
    values = [f_exact(x) for x in range(limit)]
    increasing = all(b > a for a, b in zip(values, values[1:]))
    deltas = [b - a for a, b in zip(values, values[1:])]
    concave = all(d2 < d1 for d1, d2 in zip(deltas, deltas[1:]))
    return {
        "range": f"0..{limit - 1}",
        "f0_is_0": values[0] == 0,
        "f100_is_1": abs(values[100] - 1) < Decimal(10) ** -50,
        "strictly_increasing": increasing,
        "strictly_concave_steps": concave,
        "first_thank_display_gain": canonical(1, 0)[2] - 1000,
        "first_ung_display_gain": canonical(0, 1)[2] - 1000,
        "f_at_bound": format(f_exact(BOUND - 1), ".12f"),
        "rating_at_bound_display": canonical(BOUND - 1, BOUND - 1)[2],
    }


def formula_vectors():
    """Golden corpus for the Rust fixed-point implementation (#644): the SR-01 vectors plus
    seeded pairs across magnitudes, exact powers of eleven and the domain bound."""
    rng = random.Random(644)
    pairs = [(g["impact"], g["contribution"]) for g in golden()]
    powers = [10 * 11**k - 10 for k in range(0, 12) if 10 * 11**k - 10 < BOUND]
    pairs += [(a, b) for a in powers[:6] for b in powers[:6]]
    pairs += [(powers[-1], 0), (0, powers[-1]), (powers[-1], powers[-1])]
    for upper in (2_000, 100_000, 10_000_000, BOUND - 1):
        for _ in range(500):
            pairs.append((rng.randint(0, upper), rng.randint(0, upper)))
    seen = set()
    vectors = []
    for i, c in pairs:
        if (i, c) in seen:
            continue
        seen.add((i, c))
        _, micros, display = canonical(i, c)
        vectors.append([i, c, micros, display])
    return {
        "format": "vink-reputation-formula-vectors-v1",
        "formula_version": 1,
        "generator": "services/vink-app/bench/reputation-sr01/qualify.py --vectors",
        "columns": ["impact", "contribution", "score_micros", "display"],
        "vectors": vectors,
    }


def main():
    import sys
    if sys.argv[1:] == ["--vectors"]:
        data = formula_vectors()
        lines = ",\n".join("  " + json.dumps(v) for v in data.pop("vectors"))
        header = json.dumps(data, ensure_ascii=False, indent=1)[:-2]
        print(header + ',\n "vectors": [\n' + lines + "\n ]\n}")
        return
    if sys.argv[1:]:
        raise SystemExit("usage: qualify.py [--vectors]")
    dist, sens = distribution_and_sensitivity()
    print(json.dumps({
        "policy": {
            "formula_version": 1, "base": 1000, "scale": 400, "weights": [0.6, 0.4],
            "k": 10, "reference": 100, "domain_bound_exclusive": BOUND,
            "canonical": "score_micros = floor(R * 1e6)", "display": "half-up integer of R",
            "growth": "g_tenths = 24 * dI + 16 * dC",
        },
        "shape": shape_checks(),
        "golden": golden(),
        "growth": growth(),
        "precision": precision(),
        "distribution": dist,
        "sensitivity": sens,
    }, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
